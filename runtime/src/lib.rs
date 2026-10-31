pub mod api;
mod dashboard;
pub mod domain;
pub mod fsm;
pub mod store;
pub mod tasks;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use domain::{Command, Scope, identifier, problem};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use store::{RuntimeStore, StoreError};
use subtle::ConstantTimeEq;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalConfig {
    pub id: String,
    pub token_env: String,
    pub business_id: String,
    pub project_id: String,
    pub client_id: String,
}
#[derive(Clone)]
pub struct Principal {
    pub id: String,
    pub token: String,
    pub scope: Scope,
}
#[derive(Clone)]
struct App {
    store: Arc<dyn RuntimeStore>,
    principals: Arc<Vec<Principal>>,
}
struct ApiError(StatusCode, &'static str);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.0,
            [
                (header::CONTENT_TYPE, "application/problem+json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            Json(problem(self.0.as_u16(), self.1)),
        )
            .into_response()
    }
}
impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Conflict => Self(StatusCode::CONFLICT, "idempotency_conflict"),
            StoreError::Unavailable => Self(StatusCode::SERVICE_UNAVAILABLE, "store_unavailable"),
        }
    }
}
fn auth(app: &App, h: &HeaderMap, s: &Scope) -> Result<String, ApiError> {
    let token = h
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError(StatusCode::UNAUTHORIZED, "unauthorized"))?;
    let p = app
        .principals
        .iter()
        .find(|p| bool::from(p.token.as_bytes().ct_eq(token.as_bytes())))
        .ok_or(ApiError(StatusCode::UNAUTHORIZED, "unauthorized"))?;
    if !s.valid() {
        return Err(ApiError(StatusCode::BAD_REQUEST, "invalid_scope"));
    }
    if p.scope != *s {
        return Err(ApiError(StatusCode::FORBIDDEN, "scope_denied"));
    }
    Ok(p.id.clone())
}
fn scope((business_id, project_id, client_id): (String, String, String)) -> Scope {
    Scope {
        business_id,
        project_id,
        client_id,
    }
}
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, StoreError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "store_unavailable"))?
        .map_err(Into::into)
}
async fn accept(
    State(a): State<App>,
    Path(path): Path<(String, String, String)>,
    h: HeaderMap,
    body: Result<Json<Command>, JsonRejection>,
) -> Result<Response, ApiError> {
    let s = scope(path);
    let principal = auth(&a, &h, &s)?;
    let key = h
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty() && v.len() <= 128 && v.bytes().all(|c| c.is_ascii_graphic()))
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "invalid_idempotency_key"))?
        .to_owned();
    let cmd = body
        .map_err(|e| {
            if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                ApiError(StatusCode::PAYLOAD_TOO_LARGE, "body_too_large")
            } else {
                ApiError(StatusCode::BAD_REQUEST, "invalid_json")
            }
        })?
        .0;
    if !cmd.valid() {
        return Err(ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_command_or_payload",
        ));
    }
    let r = blocking(move || a.store.accept(&s, &principal, &key, &cmd)).await?;
    let url = r["operation_url"]
        .as_str()
        .expect("store contract")
        .to_owned();
    Ok((
        StatusCode::ACCEPTED,
        [
            (header::LOCATION, url),
            (header::RETRY_AFTER, "1".into()),
            (header::CACHE_CONTROL, "no-store".into()),
        ],
        Json(r),
    )
        .into_response())
}
async fn operation(
    State(a): State<App>,
    Path((b, p, c, id)): Path<(String, String, String, String)>,
    h: HeaderMap,
) -> Result<Response, ApiError> {
    let s = scope((b, p, c));
    let principal = auth(&a, &h, &s)?;
    if !identifier(&id) {
        return Err(ApiError(StatusCode::BAD_REQUEST, "invalid_identifier"));
    }
    let v = blocking(move || a.store.operation(&s, &principal, &id))
        .await?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "not_found"))?;
    Ok(([(header::CACHE_CONTROL, "private, no-store")], Json(v)).into_response())
}
async fn object(
    State(a): State<App>,
    Path((b, p, c, id)): Path<(String, String, String, String)>,
    h: HeaderMap,
) -> Result<Response, ApiError> {
    let s = scope((b, p, c));
    auth(&a, &h, &s)?;
    if !identifier(&id) {
        return Err(ApiError(StatusCode::BAD_REQUEST, "invalid_identifier"));
    }
    let v = blocking(move || a.store.object(&s, &id))
        .await?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "not_found"))?;
    Ok((
        [
            (header::CACHE_CONTROL, "private, no-store"),
            (header::ETAG, "\"1\""),
        ],
        Json(v),
    )
        .into_response())
}

async fn accept_task(
    State(a): State<App>,
    Path(path): Path<(String, String, String)>,
    h: HeaderMap,
    body: Result<Json<tasks::TaskCommand>, JsonRejection>,
) -> Result<Response, ApiError> {
    let scope = scope(path);
    let principal = auth(&a, &h, &scope)?;
    let key = h
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty() && v.len() <= 128 && v.bytes().all(|c| c.is_ascii_graphic()))
        .ok_or(ApiError(StatusCode::BAD_REQUEST, "invalid_idempotency_key"))?
        .to_owned();
    let command = body
        .map_err(|e| {
            if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                ApiError(StatusCode::PAYLOAD_TOO_LARGE, "body_too_large")
            } else {
                ApiError(StatusCode::BAD_REQUEST, "invalid_json")
            }
        })?
        .0;
    if !command.valid() {
        return Err(ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_command_or_payload",
        ));
    }
    let result = blocking(move || a.store.accept_task(&scope, &principal, &key, &command)).await?;
    let location = result["operation_url"]
        .as_str()
        .ok_or(ApiError(StatusCode::SERVICE_UNAVAILABLE, "store_contract"))?
        .to_owned();
    Ok((
        StatusCode::ACCEPTED,
        [
            (header::LOCATION, location),
            (header::RETRY_AFTER, "1".into()),
            (header::CACHE_CONTROL, "no-store".into()),
        ],
        Json(result),
    )
        .into_response())
}
async fn task_operation(
    State(a): State<App>,
    Path((b, p, c, id)): Path<(String, String, String, String)>,
    h: HeaderMap,
) -> Result<Response, ApiError> {
    let scope = scope((b, p, c));
    let principal = auth(&a, &h, &scope)?;
    if !identifier(&id) {
        return Err(ApiError(StatusCode::BAD_REQUEST, "invalid_identifier"));
    }
    let value = blocking(move || a.store.task_operation(&scope, &principal, &id))
        .await?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "not_found"))?;
    Ok(([(header::CACHE_CONTROL, "private, no-store")], Json(value)).into_response())
}
async fn task_object(
    State(a): State<App>,
    Path((b, p, c, id)): Path<(String, String, String, String)>,
    h: HeaderMap,
) -> Result<Response, ApiError> {
    let scope = scope((b, p, c));
    auth(&a, &h, &scope)?;
    if !identifier(&id) {
        return Err(ApiError(StatusCode::BAD_REQUEST, "invalid_identifier"));
    }
    let value = blocking(move || a.store.task_object(&scope, &id))
        .await?
        .ok_or(ApiError(StatusCode::NOT_FOUND, "not_found"))?;
    Ok(([(header::CACHE_CONTROL, "private, no-store")], Json(value)).into_response())
}
async fn schema_markdown(
    State(a): State<App>,
    Path(path): Path<(String, String, String)>,
    h: HeaderMap,
) -> Result<Response, ApiError> {
    let s = scope(path);
    auth(&a, &h, &s)?;
    let body = dashboard::markdown(&s)
        .map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "contract_unavailable"))?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
            (header::CACHE_CONTROL, "private, no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        body,
    )
        .into_response())
}
async fn dashboard_html(
    State(a): State<App>,
    Path(path): Path<(String, String, String)>,
    h: HeaderMap,
) -> Result<Response, ApiError> {
    let s = scope(path);
    auth(&a, &h, &s)?;
    let body = dashboard::html(&s)
        .map_err(|_| ApiError(StatusCode::SERVICE_UNAVAILABLE, "contract_unavailable"))?;
    Ok(([(header::CONTENT_TYPE,"text/html; charset=utf-8"),(header::CACHE_CONTROL,"private, no-store"),(header::X_CONTENT_TYPE_OPTIONS,"nosniff"),(header::CONTENT_SECURITY_POLICY,"default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'")],body).into_response())
}

/// Load fixture credentials from explicitly named environment variables.
pub fn load_principals(configs: Vec<PrincipalConfig>) -> Result<Vec<Principal>, &'static str> {
    let mut principals: Vec<Principal> = Vec::new();
    for p in configs {
        let token = std::env::var(&p.token_env)
            .map_err(|_| "configured credential environment variable missing")?;
        let scope = Scope {
            business_id: p.business_id,
            project_id: p.project_id,
            client_id: p.client_id,
        };
        principals.push(Principal {
            id: p.id,
            scope,
            token,
        });
    }
    domain::validate_installed_machine()?;
    validate_principals(&principals)?;
    Ok(principals)
}

pub fn validate_principals(principals: &[Principal]) -> Result<(), &'static str> {
    if principals.is_empty() {
        return Err("principal configuration must not be empty");
    }
    for (index, p) in principals.iter().enumerate() {
        if !identifier(&p.id)
            || !p.scope.valid()
            || p.token.len() < 16
            || p.token.len() > 256
            || !p.token.bytes().all(|b| b.is_ascii_graphic())
            || principals[..index]
                .iter()
                .any(|other| other.token == p.token || other.id == p.id)
        {
            return Err("invalid or duplicate principal configuration");
        }
    }
    Ok(())
}

/// Shared HTTP contract; storage/transport adapters are injected by the host.
pub fn router(
    store: Arc<dyn RuntimeStore>,
    principals: Vec<Principal>,
) -> Result<Router, &'static str> {
    domain::validate_installed_machine()?;
    validate_principals(&principals)?;
    let app = App {
        store,
        principals: Arc::new(principals),
    };
    let router=Router::new()
        .route("/openapi.json", get(|| async { ([(header::CONTENT_TYPE, "application/json")], api::openapi_document().to_string()) }))
        .route("/version",get(||async {Json(json!({"name":"any-rs","runtime":"0.1.0-local-slice","profile":"sqlite-local","cloud_adapters_implemented":false}))}))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/commands",post(accept))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/operations/{id}",get(operation))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/demo.note/objects/{id}",get(object))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/tasks/commands",post(accept_task))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/tasks/operations/{id}",get(task_operation))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/tasks/objects/{id}",get(task_object))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/dashboard",get(dashboard_html))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/schema.md",get(schema_markdown))
        .layer(DefaultBodyLimit::max(16*1024)).with_state(app.clone());
    Ok(router)
}
