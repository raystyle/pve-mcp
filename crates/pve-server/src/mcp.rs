//! MCP ServerHandler: tool calls forward to the daemon admin plane (the
//! bridge process stays light — no client, no auth state). Three read-only
//! tools for this increment; the controlled-operation tools land next.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{
    schemars, serve_server, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::Deserialize;

use crate::client_bridge;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ClusterStatusArgs {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VmListArgs {
    /// Filter by guest type: "qemu" or "lxc" (omit for all).
    #[serde(default)]
    pub ty: Option<String>,
    /// Filter by status: "running" or "stopped" (omit for all).
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VmStatusArgs {
    /// Guest id (100..=999999), e.g. 102.
    pub vmid: i64,
}

#[derive(Clone)]
pub struct PveMcpServer {
    config_path: Option<std::path::PathBuf>,
}

impl PveMcpServer {
    pub fn with_config_path(config_path: Option<std::path::PathBuf>) -> Self {
        Self { config_path }
    }
}

#[tool_router]
impl PveMcpServer {
    /// Cluster overview: PVE version, nodes, guest counts.
    #[tool(
        description = "Proxmox VE cluster overview: version, nodes (cpu/mem/uptime), guest counts (total/running/qemu/lxc). Single-node clusters report one node."
    )]
    async fn cluster_status(
        &self,
        Parameters(_args): Parameters<ClusterStatusArgs>,
    ) -> Result<CallToolResult, McpError> {
        forward(
            self.config_path.clone(),
            "cluster_status",
            serde_json::json!({}),
        )
        .await
    }

    /// List all guests (VMs and containers) with filters.
    #[tool(
        description = "List all guests (QEMU VMs + LXC containers): vmid, name, type, status, node, template flag, memory/disk/cpu sizing. Optional filters: ty (qemu|lxc), status (running|stopped)."
    )]
    async fn vm_list(
        &self,
        Parameters(args): Parameters<VmListArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut params = serde_json::json!({});
        if let Some(ty) = args.ty {
            params["ty"] = serde_json::json!(ty);
        }
        if let Some(status) = args.status {
            params["status"] = serde_json::json!(status);
        }
        forward(self.config_path.clone(), "vm_list", params).await
    }

    /// One guest's runtime status and static config.
    #[tool(
        description = "Show one guest's details by vmid: current runtime (cpu/mem/net/uptime) and static config (cores/memory/net/disks). Locates the guest across the cluster."
    )]
    async fn vm_status(
        &self,
        Parameters(args): Parameters<VmStatusArgs>,
    ) -> Result<CallToolResult, McpError> {
        let params = serde_json::json!({"vmid": args.vmid});
        forward(self.config_path.clone(), "vm_status", params).await
    }
}

/// Forward a tool call to the daemon admin plane and wrap the reply.
/// Cold-start tolerant: no daemon listening → spawn one, wait for ready.
async fn forward(
    config_path: Option<std::path::PathBuf>,
    method: &str,
    params: serde_json::Value,
) -> Result<CallToolResult, McpError> {
    let method = method.to_string();
    let response = tokio::task::spawn_blocking(move || {
        crate::daemon::ensure_daemon(config_path.as_deref())
            .map_err(|e| anyhow::anyhow!("daemon cold start failed: {e}"))?;
        client_bridge::admin_request(&method, params)
    })
    .await
    .map_err(|e| McpError::internal_error(format!("join: {e}"), None))?
    .map_err(|e| McpError::internal_error(format!("daemon request failed: {e}"), None))?;

    if let Some(err) = response.get("error").and_then(|e| e.as_str()) {
        return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
            "error: {err}"
        ))]));
    }
    let result = &response["result"];
    let text = serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string());
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

#[tool_handler]
impl ServerHandler for PveMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "Proxmox VE single-cluster MCP tools.\n\n\
                 Start with cluster_status for the overview; vm_list to enumerate \
                 guests; vm_status for one guest's detail."
                    .to_string(),
            )
    }
}

/// Serve MCP over stdio (called from `pve-mcp stdio`).
pub async fn serve_stdio(config_path: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    // Logs to stderr only; stdout carries MCP.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let server = PveMcpServer::with_config_path(config_path);
    let service = serve_server(server, (tokio::io::stdin(), tokio::io::stdout())).await?;
    service.waiting().await?;
    Ok(())
}
