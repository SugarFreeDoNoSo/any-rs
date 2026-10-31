mod domain;
mod store;
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
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use store::{RuntimeStore, SqliteStore, StoreError};
use subtle::ConstantTimeEq;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    bind: SocketAddr,
    database: PathBuf,
    poll_ms: u64,
    principals: Vec<PrincipalConfig>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrincipalConfig {
    id: String,
    token_env: String,
    business_id: String,
    project_id: String,
    client_id: String,
}
struct Principal {
    id: String,
    token: String,
    scope: Scope,
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
#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let config_path = args
        .windows(2)
        .find(|a| a[0] == "--config")
        .map(|a| &a[1])
        .ok_or("usage: any-rs-local --config FILE [--worker-disabled]")?;
    let config: Config = toml::from_str(&std::fs::read_to_string(config_path)?)?;
    if !config.bind.ip().is_loopback()
        || !(10..=10000).contains(&config.poll_ms)
        || config.principals.is_empty()
    {
        return Err(
            "local profile requires loopback bind, principals and poll_ms=10..10000".into(),
        );
    }
    let mut principals: Vec<Principal> = Vec::new();
    for p in config.principals {
        let token = std::env::var(&p.token_env)
            .map_err(|_| "configured credential environment variable missing")?;
        let scope = Scope {
            business_id: p.business_id,
            project_id: p.project_id,
            client_id: p.client_id,
        };
        if !identifier(&p.id)
            || !scope.valid()
            || token.len() < 16
            || token.len() > 256
            || !token.bytes().all(|b| b.is_ascii_graphic())
            || principals
                .iter()
                .any(|other| other.token == token || other.id == p.id)
        {
            return Err("invalid or duplicate principal configuration".into());
        }
        principals.push(Principal {
            id: p.id,
            scope,
            token,
        });
    }
    let store =
        SqliteStore::open(&config.database).map_err(|_| "database initialization failed")?;
    let app = App {
        store: Arc::new(store),
        principals: Arc::new(principals),
    };
    let router=Router::new()
        .route("/openapi.json", get(|| async { ([(header::CONTENT_TYPE, "application/json")], include_str!("../openapi.json")) }))
        .route("/version",get(||async {Json(json!({"name":"any-rs","runtime":"0.1.0-local-slice","profile":"sqlite-local","cloud_adapters_implemented":false}))}))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/commands",post(accept))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/operations/{id}",get(operation))
        .route("/v2/businesses/{b}/projects/{p}/clients/{c}/demo.note/objects/{id}",get(object))
        .layer(DefaultBodyLimit::max(16*1024)).with_state(app.clone());
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    if !args.iter().any(|a| a == "--worker-disabled") {
        let store = app.store.clone();
        let grants = Arc::new(
            app.principals
                .iter()
                .map(|p| (p.id.clone(), p.scope.clone()))
                .collect::<Vec<_>>(),
        );
        let pause = std::time::Duration::from_millis(config.poll_ms);
        tokio::spawn(async move {
            loop {
                let s = store.clone();
                let grants = grants.clone();
                let result = tokio::task::spawn_blocking(move || s.dispatch_one(&grants)).await;
                if !matches!(result, Ok(Ok(true))) {
                    tokio::time::sleep(pause).await;
                }
            }
        });
    }
    println!("any-rs local listening {}", listener.local_addr()?);
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
