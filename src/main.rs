use anyhow::Context;

use ai_gateway::{api::server, app_version, application::AppState, config::ServerConfig};

#[cfg(unix)]
async fn wait_for_shutdown(mut terminate: tokio::signal::unix::Signal) {
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            let _ = result;
        }
        _ = terminate.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}

fn main() -> anyhow::Result<()> {
    let config = ServerConfig::from_env().context("invalid gateway configuration")?;
    let state = AppState::new(app_version());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start the Tokio runtime")?;

    runtime.block_on(async {
        #[cfg(unix)]
        let shutdown = {
            let terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .context("failed to install the SIGTERM handler")?;
            wait_for_shutdown(terminate)
        };
        #[cfg(not(unix))]
        let shutdown = wait_for_shutdown();

        server::run(config, state, shutdown).await
    })
}
