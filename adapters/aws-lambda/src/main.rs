//! Local JSONL fixture runner, explicitly NOT a Lambda bootstrap/deployment.
use any_rs_lambda::LambdaAdapter;
use any_rs_local::{PrincipalConfig, load_principals, store::SqliteStore};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    path::Path,
    sync::Arc,
};
#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("AWS_LAMBDA_FUNCTION_NAME").is_some()
        || std::env::var_os("AWS_LAMBDA_RUNTIME_API").is_some()
    {
        return Err(
            "fixture driver refuses Lambda environment; SQLite is not shared Lambda persistence"
                .into(),
        );
    }
    let args: Vec<_> = std::env::args().collect();
    let arg = |name| {
        args.windows(2)
            .find(|p| p[0] == name)
            .map(|p| p[1].clone())
            .ok_or("required --database PATH --principals JSON_PATH")
    };
    let db = arg("--database")?;
    let config = arg("--principals")?;
    let config: Vec<PrincipalConfig> = serde_json::from_str(&std::fs::read_to_string(config)?)?;
    let principals = load_principals(config)?;
    let store = SqliteStore::open(Path::new(&db)).map_err(|_| "fixture database unavailable")?;
    let adapter = LambdaAdapter::new(
        Arc::new(store),
        principals,
        "arn:aws:sqs:us-east-1:000000000000:any-rs-fixture".into(),
    )?;
    for line in io::stdin().lock().lines() {
        let result = match serde_json::from_str::<Value>(&line?) {
            Ok(mut request) => {
                let event = request["event"].take();
                match request["kind"].as_str() {
                    Some("http") => adapter.http(event).await,
                    Some("sqs") => adapter
                        .sqs(event)
                        .await
                        .unwrap_or_else(|_| json!({"error":"invalid_event"})),
                    _ => json!({"error":"invalid_event"}),
                }
            }
            Err(_) => json!({"error":"invalid_event"}),
        };
        println!("{result}");
        io::stdout().flush()?;
    }
    Ok(())
}
