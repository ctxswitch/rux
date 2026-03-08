use clap::Parser;
use tracing::info;

use rux::api;
use rux::config::Config;
use rux::storage::scylla::ScyllaStorage;
use rux::telemetry::init_telemetry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();
    init_telemetry(&config.log_level);

    info!("connecting to scylladb");
    let storage = ScyllaStorage::new(&config.scylla, config.default_consistency).await?;

    let router = api::router::build(storage, &config);

    let addr = format!("{}:{}", config.bind_address, config.port);
    info!(addr = %addr, "starting rux gateway");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    let sigterm_result = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate());

    match sigterm_result {
        Ok(mut sigterm) => {
            tokio::select! {
                _ = ctrl_c => {},
                _ = sigterm.recv() => {},
            }
        }
        Err(_) => {
            ctrl_c.await.ok();
        }
    }
    info!("shutdown signal received");
}
