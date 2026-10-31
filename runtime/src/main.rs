use any_rs_local::{
    PrincipalConfig, load_principals, router,
    store::{RuntimeStore, SqliteStore},
};
use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    bind: SocketAddr,
    database: PathBuf,
    poll_ms: u64,
    principals: Vec<PrincipalConfig>,
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
    let principals = load_principals(config.principals)?;
    let store =
        SqliteStore::open(&config.database).map_err(|_| "database initialization failed")?;
    let store: Arc<dyn RuntimeStore> = Arc::new(store);
    let grants = Arc::new(
        principals
            .iter()
            .map(|p| (p.id.clone(), p.scope.clone()))
            .collect::<Vec<_>>(),
    );
    let router = router(store.clone(), principals)?;
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    if !args.iter().any(|a| a == "--worker-disabled") {
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
