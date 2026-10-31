//! SAM-only fixture bootstrap, not a production Lambda function.
use any_rs_lambda::LambdaAdapter;
use any_rs_local::{Principal, domain::Scope, store::SqliteStore};
use lambda_runtime::{Error, LambdaEvent, service_fn};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
const TOKEN: &str = "sam-only-fixture-token-not-a-secret";
const BASE: &str = "/v2/businesses/demo/projects/notes/clients/local";
fn adapter() -> Result<(LambdaAdapter, PathBuf), Error> {
    let id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let directory = std::env::temp_dir().join(format!("any-rs-sam-{}-{id}", std::process::id()));
    std::fs::create_dir(&directory)?;
    let store = SqliteStore::open(&directory.join("fixture.sqlite3"))
        .map_err(|_| "fixture store unavailable")?;
    let app = LambdaAdapter::new(
        Arc::new(store),
        vec![Principal {
            id: "fixture".into(),
            token: TOKEN.into(),
            scope: Scope {
                business_id: "demo".into(),
                project_id: "notes".into(),
                client_id: "local".into(),
            },
        }],
        "arn:aws:sqs:us-east-1:000000000000:any-rs-fixture".into(),
    )?;
    Ok((app, directory))
}
fn http(method: &str, path: &str, body: Option<Value>) -> Value {
    json!({"version":"2.0","rawPath":path,"rawQueryString":"","headers":{"authorization":format!("Bearer {TOKEN}"),"content-type":"application/json","idempotency-key":"sam-scenario"},"requestContext":{"http":{"method":method,"path":path}},"body":body.map(|b|b.to_string()),"isBase64Encoded":false})
}
async fn scenario() -> Result<Value, Error> {
    let (app, directory) = adapter()?;
    let mut last_request = Value::Null;
    let mut last_operation = Value::Null;
    for (name, payload, key) in [
        (
            "tasks.create",
            json!({"id":"sam-task","title":"synthetic SAM task"}),
            "create",
        ),
        (
            "tasks.start",
            json!({"id":"sam-task","expected_revision":1}),
            "start",
        ),
        (
            "tasks.complete",
            json!({"id":"sam-task","expected_revision":2}),
            "complete",
        ),
    ] {
        let mut request = http(
            "POST",
            &format!("{BASE}/tasks/commands"),
            Some(json!({"name":name,"version":1,"payload":payload})),
        );
        request["headers"]["idempotency-key"] = json!(format!("sam-scenario-{key}"));
        let accepted = app.http(request.clone()).await;
        if accepted["statusCode"] != 202 {
            return Err("scenario task acceptance failed".into());
        }
        let op: Value =
            serde_json::from_str(accepted["body"].as_str().ok_or("missing response body")?)?;
        let before = app
            .http(http(
                "GET",
                op["operation_url"]
                    .as_str()
                    .ok_or("missing operation URL")?,
                None,
            ))
            .await;
        let before: Value =
            serde_json::from_str(before["body"].as_str().ok_or("missing response body")?)?;
        if before["status"] != "accepted" {
            return Err("unexpected post-response worker".into());
        }
        let work = json!({"Records":[{"messageId":format!("sam-{key}"),"eventSource":"aws:sqs","eventSourceARN":"arn:aws:sqs:us-east-1:000000000000:any-rs-fixture","awsRegion":"us-east-1","body":json!({"operation_id":op["operation_id"],"principal_id":"fixture","scope":{"business_id":"demo","project_id":"notes","client_id":"local"}}).to_string()}]});
        if app.sqs(work.clone()).await?["batchItemFailures"] != json!([])
            || app.sqs(work).await?["batchItemFailures"] != json!([])
        {
            return Err("scenario worker/retry failed".into());
        }
        let completed = app
            .http(http(
                "GET",
                op["operation_url"]
                    .as_str()
                    .ok_or("missing operation URL")?,
                None,
            ))
            .await;
        let completed: Value =
            serde_json::from_str(completed["body"].as_str().ok_or("missing operation body")?)?;
        if completed["status"] != "succeeded" {
            return Err("scenario task transition failed".into());
        }
        last_request = request;
        last_operation = op;
    }
    let replay = app.http(last_request).await;
    let replay: Value =
        serde_json::from_str(replay["body"].as_str().ok_or("missing replay body")?)?;
    if replay["operation_id"] != last_operation["operation_id"] || replay["status"] != "succeeded" {
        return Err("scenario idempotency failed".into());
    }
    let response = app
        .http(http("GET", &format!("{BASE}/tasks/objects/sam-task"), None))
        .await;
    if response["statusCode"] != 200 {
        return Err("scenario task missing".into());
    }
    let object: Value =
        serde_json::from_str(response["body"].as_str().ok_or("missing task body")?)?;
    if object["state"] != "completed" || object["revision"] != 3 {
        return Err("scenario terminal state invalid".into());
    }
    drop(app);
    std::fs::remove_dir_all(directory)?;
    Ok(
        json!({"status":"passed","profile":"sam-local-fixture-only","checks":["explicit-creation-pending","declared-start","declared-complete","accepted-before-worker","terminal-replay","idempotency","object-read"],"object":object,"aws_services_contacted":false,"cross_container_durability_tested":false}),
    )
}
#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Error> {
    let local = std::env::args().any(|a| a == "--scenario-local");
    if local
        && std::env::var_os("AWS_LAMBDA_FUNCTION_NAME").is_none()
        && std::env::var_os("AWS_LAMBDA_RUNTIME_API").is_none()
    {
        println!("{}", scenario().await?);
        return Ok(());
    }
    if std::env::var("AWS_SAM_LOCAL").as_deref() != Ok("true")
        || std::env::var("ANYRS_SAM_FIXTURE_ONLY").as_deref() != Ok("true")
    {
        return Err("SAM fixture refuses cloud runtime: AWS_SAM_LOCAL=true and ANYRS_SAM_FIXTURE_ONLY=true required".into());
    }
    if std::env::var("ANYRS_SAM_MODE").as_deref() == Ok("scenario") {
        lambda_runtime::run(service_fn(|_: LambdaEvent<Value>| async {
            scenario().await
        }))
        .await
    } else {
        let (app, _directory) = adapter()?;
        // Warm reuse is a convenience, not shared durable storage. No background
        // task drains accepted work after the HTTP invocation returns.
        lambda_runtime::run(service_fn(move |event: LambdaEvent<Value>| {
            let app = app.clone();
            async move {
                if event.payload.get("Records").is_some() {
                    app.sqs(event.payload).await
                } else {
                    Ok::<Value, Error>(app.http(event.payload).await)
                }
            }
        }))
        .await
    }
}
