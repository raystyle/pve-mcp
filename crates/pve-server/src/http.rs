//! Streamable HTTP MCP transport (know-mcp heritage): served by a light
//! bridge process that forwards tool calls to the daemon; the daemon
//! stays the single owner of the PVE client.

use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};

pub async fn serve(config_path: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    let config = pve_core::config::Config::load(config_path.as_deref())?;
    let bind = config.server.http_bind.clone();
    drop(config);

    let config_path = config_path.clone();
    let service = StreamableHttpService::new(
        move || {
            Ok(crate::mcp::PveMcpServer::with_config_path(
                config_path.clone(),
            ))
        },
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    eprintln!("MCP streamable HTTP listening on http://{bind}/mcp");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
