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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VmPowerArgs {
    /// Guest id (100..=999999).
    pub vmid: i64,
    /// One of: start | shutdown | stop | reboot | reset | suspend | resume.
    pub action: String,
    /// Required for shutdown/stop/reboot/reset/suspend (start/resume ungated).
    #[serde(default)]
    pub confirm: Option<bool>,
    /// Wait for the task to finish (default true).
    #[serde(default)]
    pub wait: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VmCreateArgs {
    /// Name for the new VM (no whitespace, no '/').
    pub name: String,
    /// Source template vmid (must be a qemu template).
    pub template: i64,
    /// Explicit new vmid; omit to take the cluster's next free id.
    #[serde(default)]
    pub new_vmid: Option<i64>,
    /// Dry-run until true.
    #[serde(default)]
    pub confirm: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VmDeleteArgs {
    /// Guest id to destroy.
    pub vmid: i64,
    /// Dry-run until true. Running guests are refused even with confirm.
    #[serde(default)]
    pub confirm: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VmMetricsArgs {
    /// Guest id.
    pub vmid: i64,
    /// RRD timeframe: hour | day | week | month | year.
    #[serde(default)]
    pub timeframe: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VmAgentArgs {
    /// Guest id (running qemu guest with agent enabled).
    pub vmid: i64,
    /// ping | osinfo | interfaces (IP discovery).
    #[serde(default)]
    pub what: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StorageListArgs {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TaskStatusArgs {
    /// Task UPID (embeds the node).
    pub upid: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BackupManageArgs {
    /// list | run | restore.
    pub action: String,
    /// Guest id (run).
    #[serde(default)]
    pub vmid: Option<i64>,
    /// Storage name (run default local; list filters).
    #[serde(default)]
    pub storage: Option<String>,
    /// Backup volid to restore (restore).
    #[serde(default)]
    pub archive: Option<String>,
    /// New vmid for the restored guest (restore; never in-place).
    #[serde(default)]
    pub new_vmid: Option<i64>,
    /// Dry-run until true (run/restore).
    #[serde(default)]
    pub confirm: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FirewallManageArgs {
    /// Guest id.
    pub vmid: i64,
    /// list | create | delete.
    pub action: String,
    /// ACCEPT | DROP | REJECT (create).
    #[serde(default, rename = "action_rule")]
    pub action_rule: Option<String>,
    /// in | out (create).
    #[serde(default)]
    pub ty: Option<String>,
    /// Destination port (create).
    #[serde(default)]
    pub dport: Option<String>,
    /// tcp | udp | icmp (create).
    #[serde(default)]
    pub proto: Option<String>,
    /// Source address/CIDR (create).
    #[serde(default)]
    pub source: Option<String>,
    /// Rule comment (create).
    #[serde(default)]
    pub comment: Option<String>,
    /// Rule position from list (delete).
    #[serde(default)]
    pub pos: Option<i64>,
    /// Dry-run until true (create/delete).
    #[serde(default)]
    pub confirm: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SnapshotManageArgs {
    /// Guest id.
    pub vmid: i64,
    /// list | create | rollback | delete.
    pub action: String,
    /// Snapshot name (create/rollback/delete; not needed for list).
    #[serde(default)]
    pub name: Option<String>,
    /// Optional description (create only).
    #[serde(default)]
    pub description: Option<String>,
    /// Required for rollback and delete (create is dry-run without it).
    #[serde(default)]
    pub confirm: Option<bool>,
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

    /// Power actions with idempotence and confirm gating.
    #[tool(
        description = "Power action on a guest. Actions: start, shutdown (ACPI), stop (hard cut), reboot, reset, suspend, resume. Idempotent: an action whose target state is already reached is skipped without writing. shutdown/stop/reboot/reset/suspend require confirm=true. Waits for the task by default (wait=false returns the UPID immediately).",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true
        )
    )]
    async fn vm_power(
        &self,
        Parameters(args): Parameters<VmPowerArgs>,
    ) -> Result<CallToolResult, McpError> {
        let params = serde_json::json!({
            "vmid": args.vmid,
            "action": args.action,
            "confirm": args.confirm.unwrap_or(false),
            "wait": args.wait.unwrap_or(true),
        });
        forward(self.config_path.clone(), "vm_power", params).await
    }

    /// Clone a template into a new VM (dry-run by default).
    #[tool(
        description = "Clone a QEMU template into a new VM. Dry-run by default: without confirm=true it returns the plan (next free vmid, source node, name) without writing. confirm=true executes the clone and waits for it. Name must not contain whitespace or '/'.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    async fn vm_create(
        &self,
        Parameters(args): Parameters<VmCreateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut params = serde_json::json!({
            "template": args.template,
            "name": args.name,
            "confirm": args.confirm.unwrap_or(false),
        });
        if let Some(new_vmid) = args.new_vmid {
            params["new_vmid"] = serde_json::json!(new_vmid);
        }
        forward(self.config_path.clone(), "vm_create", params).await
    }

    /// Destroy a stopped guest, purging disks (dry-run by default).
    #[tool(
        description = "Destroy a guest and purge its disks. Dry-run by default (returns the plan). Refuses running guests even with confirm=true — stop first. protected_vmids from config are refused unconditionally. confirm=true executes and waits.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false
        )
    )]
    async fn vm_delete(
        &self,
        Parameters(args): Parameters<VmDeleteArgs>,
    ) -> Result<CallToolResult, McpError> {
        let params = serde_json::json!({
            "vmid": args.vmid,
            "confirm": args.confirm.unwrap_or(false),
        });
        forward(self.config_path.clone(), "vm_delete", params).await
    }

    /// Snapshot operations for one guest (list/create/rollback/delete).
    #[tool(
        description = "Manage guest snapshots. Actions: list (name/time/vmstate/parent), create (name + optional description; dry-run until confirm=true), rollback (restores the guest to the snapshot; destructive — needs confirm=true; PVE stops a running guest when vmstate was captured), delete (destructive — needs confirm=true). Names must not contain whitespace, '/', ',' or '='; 'current' is rejected.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false
        )
    )]
    async fn snapshot_manage(
        &self,
        Parameters(args): Parameters<SnapshotManageArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut params = serde_json::json!({
            "vmid": args.vmid,
            "action": args.action,
            "confirm": args.confirm.unwrap_or(false),
        });
        if let Some(name) = &args.name {
            params["name"] = serde_json::json!(name);
        }
        if let Some(desc) = &args.description {
            params["description"] = serde_json::json!(desc);
        }
        forward(self.config_path.clone(), "snapshot_manage", params).await
    }

    /// RRD metrics for one guest.
    #[tool(
        description = "Historical metrics (RRD) for one guest: cpu, memory, network, disk over the chosen timeframe (hour|day|week|month|year, default hour). Rows are evenly down-sampled to keep the reply compact. Read-only.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn vm_metrics(
        &self,
        Parameters(args): Parameters<VmMetricsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let params = serde_json::json!({
            "vmid": args.vmid,
            "timeframe": args.timeframe.unwrap_or_else(|| "hour".to_string()),
        });
        forward(self.config_path.clone(), "vm_metrics", params).await
    }

    /// QGA read surface: ping / osinfo / network interfaces.
    #[tool(
        description = "QEMU guest agent read queries for a running VM. what: ping (agent reachable?), osinfo (OS name/version), interfaces (network interfaces with IP addresses). qemu guests only; requires guest agent enabled. Read-only.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn vm_agent(
        &self,
        Parameters(args): Parameters<VmAgentArgs>,
    ) -> Result<CallToolResult, McpError> {
        let params = serde_json::json!({
            "vmid": args.vmid,
            "what": args.what.unwrap_or_else(|| "ping".to_string()),
        });
        forward(self.config_path.clone(), "vm_agent", params).await
    }

    /// Storage definitions + usage.
    #[tool(
        description = "List storages with per-node usage (total/used GB). Read-only.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn storage_list(
        &self,
        Parameters(_args): Parameters<StorageListArgs>,
    ) -> Result<CallToolResult, McpError> {
        forward(
            self.config_path.clone(),
            "storage_list",
            serde_json::json!({}),
        )
        .await
    }

    /// Task status + log tail by UPID.
    #[tool(
        description = "Status and log tail of one task by UPID (returned by any long-running operation). Read-only.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn task_status(
        &self,
        Parameters(args): Parameters<TaskStatusArgs>,
    ) -> Result<CallToolResult, McpError> {
        let params = serde_json::json!({"upid": args.upid});
        forward(self.config_path.clone(), "task_status", params).await
    }

    /// vzdump backup run / list / restore.
    #[tool(
        description = "vzdump backups. Actions: list (backup archives on a storage), run (snapshot-mode vzdump of one guest; dry-run until confirm=true, waits for the task), restore (creates a NEW vmid from an archive — never in-place; dry-run until confirm=true; waits). protected_vmids apply to run targets and restore target ids.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false
        )
    )]
    async fn backup_manage(
        &self,
        Parameters(args): Parameters<BackupManageArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut params = serde_json::json!({
            "action": args.action,
            "confirm": args.confirm.unwrap_or(false),
        });
        if let Some(v) = args.vmid {
            params["vmid"] = serde_json::json!(v);
        }
        if let Some(v) = &args.storage {
            params["storage"] = serde_json::json!(v);
        }
        if let Some(v) = &args.archive {
            params["archive"] = serde_json::json!(v);
        }
        if let Some(v) = args.new_vmid {
            params["new_vmid"] = serde_json::json!(v);
        }
        forward(self.config_path.clone(), "backup_manage", params).await
    }

    /// Guest firewall rules (list/create/delete).
    #[tool(
        description = "Guest-level firewall rules (per-VM). Actions: list (rules with positions), create (rule fields: action_rule ACCEPT|DROP|REJECT, ty in|out, dport, proto, source, comment; dry-run until confirm=true), delete (by pos from list; dry-run until confirm=true). protected_vmids apply to create/delete.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false
        )
    )]
    async fn firewall_manage(
        &self,
        Parameters(args): Parameters<FirewallManageArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut params = serde_json::json!({
            "vmid": args.vmid,
            "action": args.action,
            "confirm": args.confirm.unwrap_or(false),
        });
        if let Some(v) = &args.action_rule {
            params["action_rule"] = serde_json::json!(v);
        }
        if let Some(v) = &args.ty {
            params["ty"] = serde_json::json!(v);
        }
        if let Some(v) = &args.dport {
            params["dport"] = serde_json::json!(v);
        }
        if let Some(v) = &args.proto {
            params["proto"] = serde_json::json!(v);
        }
        if let Some(v) = &args.source {
            params["source"] = serde_json::json!(v);
        }
        if let Some(v) = &args.comment {
            params["comment"] = serde_json::json!(v);
        }
        if let Some(v) = args.pos {
            params["pos"] = serde_json::json!(v);
        }
        forward(self.config_path.clone(), "firewall_manage", params).await
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
