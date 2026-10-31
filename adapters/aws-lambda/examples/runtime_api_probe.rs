//! Test-only Runtime API probe. Refuses real Lambda and non-loopback endpoints.
use any_rs_lambda::LambdaAdapter;
use any_rs_local::{PrincipalConfig, load_principals, store::SqliteStore};
use std::{net::SocketAddr, path::Path, sync::Arc};

fn main() -> Result<(), lambda_runtime::Error> {
    if std::env::var_os("AWS_LAMBDA_FUNCTION_NAME").is_some() {
        return Err("test probe refuses Lambda function environment".into());
    }
    let endpoint: SocketAddr = std::env::var("AWS_LAMBDA_RUNTIME_API")?.parse()?;
    if !endpoint.ip().is_loopback() || endpoint.port() == 0 {
        return Err("test probe requires a loopback Runtime API socket".into());
    }
    let args: Vec<_> = std::env::args().collect();
    let arg = |name| {
        args.windows(2)
            .find(|p| p[0] == name)
            .map(|p| p[1].clone())
            .ok_or("required --database PATH --principals JSON_PATH --mode http|sqs")
    };
    let mode = arg("--mode")?;
    if mode != "http" && mode != "sqs" {
        return Err("invalid probe mode".into());
    }
    let db = arg("--database")?;
    let config = arg("--principals")?;
    let config: Vec<PrincipalConfig> = serde_json::from_str(&std::fs::read_to_string(config)?)?;
    let store = SqliteStore::open(Path::new(&db)).map_err(|_| "fixture database unavailable")?;
    let adapter = LambdaAdapter::new(
        Arc::new(store),
        load_principals(config)?,
        "arn:aws:sqs:us-east-1:000000000000:any-rs-fixture".into(),
    )?;
    // SAFETY: this test executable is still single-threaded; the Tokio runtime
    // is constructed only below. Reject inherited Lambda identity above, then
    // synthesize the three configuration fields required by the actual SDK.
    unsafe {
        std::env::set_var("AWS_LAMBDA_FUNCTION_NAME", "local-runtime-api-probe");
        std::env::set_var("AWS_LAMBDA_FUNCTION_MEMORY_SIZE", "128");
        std::env::set_var("AWS_LAMBDA_FUNCTION_VERSION", "$LATEST");
    }
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?
        .block_on(async move {
            if mode == "http" {
                adapter.run_http().await
            } else {
                adapter.run_sqs().await
            }
        })
}
