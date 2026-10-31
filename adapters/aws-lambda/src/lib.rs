//! AWS invocation adapters, not AWS persistence or transport implementations.
//! Production callers must inject shared durable storage; the only executable
//! provided here is an explicitly local fixture driver.
use any_rs_local::{
    Principal,
    domain::{Scope, identifier, problem},
    store::RuntimeStore,
};
use aws_lambda_events::event::{apigw::ApiGatewayV2httpRequest, sqs::SqsEvent};
use axum::{
    Router,
    body::Body,
    http::{Method, Request},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use http_body_util::BodyExt;
use lambda_runtime::{Error, LambdaEvent, service_fn};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};
use tower::ServiceExt;

const MAX_BODY: usize = 16 * 1024;
#[derive(Clone)]
pub struct LambdaAdapter {
    router: Router,
    store: Arc<dyn RuntimeStore>,
    grants: Arc<Vec<(String, Scope)>>,
    queue_arn: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkReference {
    operation_id: String,
    principal_id: String,
    scope: Scope,
}
fn response(status: u16, body: Value) -> Value {
    json!({"statusCode":status,"headers":{"content-type":"application/problem+json","cache-control":"no-store"},"body":body.to_string(),"isBase64Encoded":false})
}
fn invalid(code: &str) -> Value {
    response(400, problem(400, code))
}
impl LambdaAdapter {
    /// `queue_arn` is an exact allowlist for one standard SQS queue. It is not
    /// authentication: deployment IAM/event-source permissions are also required.
    pub fn new(
        store: Arc<dyn RuntimeStore>,
        principals: Vec<Principal>,
        queue_arn: String,
    ) -> Result<Self, &'static str> {
        let arn: Vec<_> = queue_arn.split(':').collect();
        if arn.len() != 6
            || arn[0] != "arn"
            || arn[1] != "aws"
            || arn[2] != "sqs"
            || arn[3].is_empty()
            || arn[4].len() != 12
            || !arn[4].bytes().all(|b| b.is_ascii_digit())
            || arn[5].is_empty()
            || arn[5].ends_with(".fifo")
        {
            return Err("standard SQS ARN required");
        }
        let grants = Arc::new(
            principals
                .iter()
                .map(|p| (p.id.clone(), p.scope.clone()))
                .collect(),
        );
        let router = any_rs_local::router(store.clone(), principals)?;
        Ok(Self {
            router,
            store,
            grants,
            queue_arn,
        })
    }
    /// Exactly one invocation. No background worker is started or awaited later.
    pub async fn http(&self, event: Value) -> Value {
        if event["version"] != "2.0" || !event["requestContext"]["http"]["method"].is_string() {
            return invalid("unsupported_gateway_event");
        }
        // HTTP v2 combines duplicate headers with commas. Reject ambiguity before
        // deserializing HeaderMap, including differently-cased duplicate keys.
        let Some(headers) = event["headers"].as_object() else {
            return invalid("invalid_headers");
        };
        let mut seen = HashSet::new();
        for (name, value) in headers {
            let lower = name.to_ascii_lowercase();
            let Some(value) = value.as_str() else {
                return invalid("invalid_headers");
            };
            if !seen.insert(lower.clone())
                || (["authorization", "idempotency-key"].contains(&lower.as_str())
                    && value.contains(','))
            {
                return invalid("ambiguous_headers");
            }
        }
        let Ok(event) = serde_json::from_value::<ApiGatewayV2httpRequest>(event) else {
            return invalid("invalid_gateway_event");
        };
        let Some(path) = event.raw_path else {
            return invalid("invalid_path");
        };
        if !path.starts_with('/')
            || path.starts_with("//")
            || path.contains('?')
            || path.contains('#')
            || event
                .request_context
                .http
                .path
                .as_ref()
                .is_some_and(|p| p != &path)
        {
            return invalid("invalid_path");
        }
        if event
            .raw_query_string
            .as_ref()
            .is_some_and(|q| !q.is_empty())
        {
            return invalid("query_not_supported");
        }
        let raw = event.body.unwrap_or_default();
        if raw.len() > MAX_BODY * 2 {
            return response(413, problem(413, "body_too_large"));
        }
        let body = if event.is_base64_encoded {
            match STANDARD.decode(raw) {
                Ok(b) => b,
                Err(_) => return invalid("invalid_base64"),
            }
        } else {
            raw.into_bytes()
        };
        if body.len() > MAX_BODY {
            return response(413, problem(413, "body_too_large"));
        }
        let method = event.request_context.http.method;
        if method == Method::GET && path == "/version" {
            return json!({"statusCode":200,"headers":{"content-type":"application/json","cache-control":"no-store"},"body":json!({"name":"any-rs","profile":"aws-lambda-invocation-adapter","gateway_payload":"2.0","aws_persistence_implemented":false,"sqs_transport_implemented":false}).to_string(),"isBase64Encoded":false});
        }
        let Ok(mut request) = Request::builder()
            .method(method)
            .uri(path)
            .body(Body::from(body))
        else {
            return invalid("invalid_request");
        };
        *request.headers_mut() = event.headers;
        let result = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("infallible Axum router");
        let (parts, body) = result.into_parts();
        let headers: BTreeMap<String, String> = parts
            .headers
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_owned())))
            .collect();
        let Ok(body) = body.collect().await else {
            return response(503, problem(503, "response_unavailable"));
        };
        let Ok(body) = String::from_utf8(body.to_bytes().to_vec()) else {
            return response(503, problem(503, "response_encoding"));
        };
        json!({"statusCode":parts.status.as_u16(),"headers":headers,"body":body,"isBase64Encoded":false})
    }
    /// Standard SQS batches only. Return after each durable dispatch has completed.
    /// Configure ReportBatchItemFailures on the future event source mapping.
    pub async fn sqs(&self, event: Value) -> Result<Value, Error> {
        let event: SqsEvent = serde_json::from_value(event)?;
        if event.records.is_empty() || event.records.len() > 10 {
            return Err("invalid batch size (local profile supports 1..10)".into());
        }
        let mut seen = HashSet::new();
        // Preflight all IDs, since there is no valid partial-failure identifier
        // for a missing ID. Never partially execute an unreportable batch.
        for m in &event.records {
            let id = m
                .message_id
                .as_ref()
                .filter(|s| !s.is_empty())
                .ok_or("missing message ID")?;
            if !seen.insert(id.clone()) {
                return Err("duplicate message ID".into());
            }
        }
        let mut failures = Vec::new();
        for m in event.records {
            let id = m.message_id.expect("preflight");
            let source_ok = m.event_source.as_deref() == Some("aws:sqs")
                && m.event_source_arn.as_deref() == Some(&self.queue_arn)
                && m.aws_region.as_deref() == self.queue_arn.split(':').nth(3);
            let work = m
                .body
                .filter(|b| b.len() <= MAX_BODY)
                .and_then(|b| serde_json::from_str::<WorkReference>(&b).ok());
            let result = if let Some(w) = work.filter(|w| {
                source_ok
                    && w.scope.valid()
                    && identifier(&w.operation_id)
                    && identifier(&w.principal_id)
            }) {
                let store = self.store.clone();
                let grants = self.grants.clone();
                tokio::task::spawn_blocking(move || {
                    store.dispatch_operation(&w.scope, &w.principal_id, &w.operation_id, &grants)
                })
                .await
                .is_ok_and(|r| r.is_ok())
            } else {
                false
            };
            if !result {
                failures.push(json!({"itemIdentifier":id}));
            }
        }
        Ok(json!({"batchItemFailures":failures}))
    }
    /// Runtime API entrypoint for a host that supplies real durable adapters.
    /// This crate deliberately ships no deployable SQLite-backed bootstrap.
    pub async fn run_http(self) -> Result<(), Error> {
        lambda_runtime::run(service_fn(move |event: LambdaEvent<Value>| {
            let app = self.clone();
            async move { Ok::<_, Error>(app.http(event.payload).await) }
        }))
        .await
    }
    pub async fn run_sqs(self) -> Result<(), Error> {
        lambda_runtime::run(service_fn(move |event: LambdaEvent<Value>| {
            let app = self.clone();
            async move { app.sqs(event.payload).await }
        }))
        .await
    }
}
