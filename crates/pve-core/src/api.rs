//! Typed query methods over the PVE API (read tools' data layer).

use serde::Deserialize;

use crate::client::urlencoding_encode;
use crate::client::{ApiError, Method, PveClient};
use crate::domain::{GuestRef, GuestType, NodeName, SnapshotName, StorageName, Vmid};

/// One guest row from /cluster/resources?type=vm (qemu + lxc together —
/// the caller filters by `ty`).
#[derive(Debug, Clone, Deserialize)]
pub struct GuestSummary {
    pub vmid: u32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub node: Option<String>,
    #[serde(default)]
    pub maxmem: Option<u64>,
    #[serde(default)]
    pub maxdisk: Option<u64>,
    #[serde(default)]
    pub maxcpu: Option<f64>,
    #[serde(default)]
    pub cpu: Option<f64>,
    #[serde(default)]
    pub mem: Option<u64>,
    #[serde(default)]
    pub uptime: Option<u64>,
    #[serde(default)]
    pub template: Option<u64>,
}

impl GuestSummary {
    pub fn is_template(&self) -> bool {
        self.template.unwrap_or(0) == 1
    }

    pub fn guest_type(&self) -> Option<GuestType> {
        match self.ty.as_str() {
            "qemu" => Some(GuestType::Qemu),
            "lxc" => Some(GuestType::Lxc),
            _ => None,
        }
    }
}

/// One node row from /nodes.
#[derive(Debug, Clone, Deserialize)]
pub struct NodeSummary {
    pub node: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub cpu: Option<f64>,
    #[serde(default)]
    pub maxcpu: Option<u64>,
    #[serde(default)]
    pub mem: Option<u64>,
    #[serde(default)]
    pub maxmem: Option<u64>,
    #[serde(default)]
    pub uptime: Option<u64>,
}

/// Storage definition row (GET /storage).
#[derive(Debug, Clone, Deserialize)]
pub struct StorageDef {
    pub storage: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub nodes: Option<String>,
}

/// Per-node storage usage row (GET /nodes/{node}/storage).
#[derive(Debug, Clone, Deserialize)]
pub struct StorageUsage {
    pub storage: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub used: Option<u64>,
    #[serde(default)]
    pub active: Option<u64>,
}

/// Task row (GET /nodes/{node}/tasks).
#[derive(Debug, Clone, Deserialize)]
pub struct TaskRow {
    #[serde(default)]
    pub upid: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(rename = "type", default)]
    pub worker_type: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub starttime: Option<i64>,
    #[serde(default)]
    pub endtime: Option<i64>,
}

/// Result of an SSH-channel exec (pct exec via host).
#[derive(Debug, Clone)]
pub struct SshExecOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// One snapshot row (post `current` filtering).
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotEntry {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub snaptime: Option<i64>,
    #[serde(default)]
    pub vmstate: Option<String>,
    #[serde(default)]
    pub parent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    #[serde(default)]
    pub release: Option<String>,
    #[serde(default)]
    pub repoid: Option<String>,
}

impl PveClient {
    pub fn api_version(&mut self) -> Result<Option<VersionInfo>, ApiError> {
        self.get_json("/version")
    }

    pub fn cluster_nodes(&mut self) -> Result<Option<Vec<NodeSummary>>, ApiError> {
        self.get_json("/nodes")
    }

    pub fn cluster_guests(&mut self) -> Result<Option<Vec<GuestSummary>>, ApiError> {
        self.get_json("/cluster/resources?type=vm")
    }

    /// Locate one guest by vmid across the cluster (single node today, but
    /// the scan keeps the contract cluster-shaped). Errors when absent.
    pub fn locate_guest(&mut self, vmid: Vmid) -> Result<GuestRef, ApiError> {
        let guests = self.cluster_guests()?.unwrap_or_default();
        for guest in guests {
            if guest.vmid != vmid.get() {
                continue;
            }
            let Some(ty) = guest.guest_type() else {
                continue;
            };
            let Some(node) = guest.node.clone() else {
                continue;
            };
            let node = NodeName::try_from(node)
                .map_err(|e| ApiError::Parse("/cluster/resources".to_string(), e.to_string()))?;
            return Ok(GuestRef::new(node, ty, vmid));
        }
        Err(ApiError::Parse(
            "/cluster/resources".to_string(),
            format!("no guest with vmid {vmid}"),
        ))
    }

    /// GET status/current for one guest (runtime: cpu/mem/net/disk/uptime).
    pub fn guest_current(
        &mut self,
        guest: &GuestRef,
    ) -> Result<Option<serde_json::Value>, ApiError> {
        self.get_json(&format!("/{}/status/current", guest.api_path()))
    }

    /// GET config for one guest (static: cores/memory/net/disks).
    pub fn guest_config(
        &mut self,
        guest: &GuestRef,
    ) -> Result<Option<serde_json::Value>, ApiError> {
        self.get_json(&format!("/{}/config", guest.api_path()))
    }

    /// Wait for a task to reach a terminal state. Polls status; on failure
    /// appends the last log lines. Returns (ok, status_text, log_tail).
    pub fn wait_task(
        &mut self,
        node: &NodeName,
        upid: &crate::domain::Upid,
        timeout: std::time::Duration,
    ) -> Result<(bool, String, Vec<String>), ApiError> {
        let path = format!("/nodes/{}/tasks/{}/status", node, upid);
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let status: serde_json::Value =
                self.get_json(&path)?.unwrap_or(serde_json::Value::Null);
            let exit = status.get("exitstatus").and_then(|v| v.as_str());
            match exit {
                Some("OK") => return Ok((true, "OK".to_string(), Vec::new())),
                Some(err) => {
                    let log = self.task_log_tail(node, upid, 20)?;
                    return Ok((false, err.to_string(), log));
                }
                None if std::time::Instant::now() >= deadline => {
                    return Ok((false, "timeout".to_string(), Vec::new()));
                }
                None => std::thread::sleep(std::time::Duration::from_millis(500)),
            }
        }
    }

    pub fn task_log_tail(
        &mut self,
        node: &NodeName,
        upid: &crate::domain::Upid,
        limit: usize,
    ) -> Result<Vec<String>, ApiError> {
        let path = format!("/nodes/{}/tasks/{}/log?limit={}", node, upid, limit);
        let lines: Option<Vec<serde_json::Value>> = self.get_json(&path)?;
        Ok(lines
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| v.get("t").and_then(|t| t.as_str()).map(String::from))
            .collect())
    }

    /// Form-POST helper for endpoints taking simple string fields (kept
    /// pub for the controlled-operation tools).
    pub fn post_fields(
        &mut self,
        path: &str,
        fields: &[(String, String)],
    ) -> Result<Option<serde_json::Value>, ApiError> {
        self.post_form(path, fields)
    }

    /// Raw DELETE with query args (controlled-operation tools).
    pub fn delete_query(
        &mut self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<Option<serde_json::Value>, ApiError> {
        self.delete(path, query)
    }

    // -----------------------------------------------------------------------
    // Controlled-operation data layer. NO guardrail logic lives here —
    // confirm gating, idempotence, protected_vmids are daemon-side policy.
    // -----------------------------------------------------------------------

    /// Next free cluster vmid (GET /cluster/nextid). Accepts numeric and
    /// string forms (PVE has returned both across versions).
    pub fn next_vmid(&mut self) -> Result<Option<u32>, ApiError> {
        let raw: serde_json::Value = self
            .get_json("/cluster/nextid")?
            .unwrap_or(serde_json::Value::Null);
        let id = raw
            .as_u64()
            .or_else(|| raw.as_str().and_then(|s| s.parse::<u64>().ok()));
        Ok(id.map(|v| v as u32))
    }

    /// Power action POST. The daemon layer decides whether the call is
    /// idempotent-skipped; this only executes. Returns the task UPID when
    /// PVE reports one.
    pub fn power_action(
        &mut self,
        guest: &GuestRef,
        action: PowerAction,
    ) -> Result<Option<String>, ApiError> {
        let path = format!("/{}/status/{}", guest.api_path(), action.as_str());
        self.post_form(&path, &[])
    }

    /// Current runtime status string for one guest ("running"/"stopped").
    pub fn guest_status_str(&mut self, guest: &GuestRef) -> Result<String, ApiError> {
        let path = format!("/{}/status/current", guest.api_path());
        let value: serde_json::Value = self.get_json(&path)?.unwrap_or(serde_json::Value::Null);
        Ok(value
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string())
    }

    /// QGA guest exec (async two-step, ry-ops form): POST exec returns a
    /// PID; poll exec-status for exit code + output. Payloads larger than
    /// ~3k chars are rejected here — the QGA argv has a hard ~8KiB
    /// window with base64 UTF-16LE truncation beyond that (M035).
    pub fn agent_exec_start(
        &mut self,
        guest: &GuestRef,
        command: &str,
        args: &[&str],
    ) -> Result<Option<serde_json::Value>, ApiError> {
        if command.len() + args.iter().map(|a| a.len()).sum::<usize>() > 2800 {
            return Err(ApiError::Status {
                status: 413,
                method: "POST",
                path: guest.api_path() + "/agent/exec",
                reason: "QGA payload too large (keep command + args under ~2800 chars; the argv channel truncates beyond that)".to_string(),
            });
        }
        let path = format!("/{}/agent/exec", guest.api_path());
        // PVE 9 schema: command is an ARRAY — program + arguments as one
        // repeated form key (pvesh usage: "--command <array>"). Separate
        // argN/arguments keys 400 with "not defined in schema".
        let mut fields: Vec<(String, String)> = vec![("command".to_string(), command.to_string())];
        for arg in args {
            fields.push(("command".to_string(), arg.to_string()));
        }
        self.post_form(&path, &fields)
    }

    /// Poll a guest exec by its pid; returns the exec-status object
    /// (exited, exitcode, out-data/out-err as base64).
    pub fn agent_exec_status(
        &mut self,
        guest: &GuestRef,
        pid: u64,
    ) -> Result<serde_json::Value, ApiError> {
        let path = format!("/{}/agent/exec-status?pid={}", guest.api_path(), pid);
        Ok(self.get_json(&path)?.unwrap_or(serde_json::Value::Null))
    }

    /// Update guest config (PUT config with provided key=value fields).
    /// Atomic per PVE: one bad field rejects the whole write (M015) —
    /// callers must re-read config after any failure.
    pub fn guest_set_config(
        &mut self,
        guest: &GuestRef,
        fields: &[(String, String)],
    ) -> Result<Option<String>, ApiError> {
        let path = format!("/{}/config", guest.api_path());
        self.put_form(&path, fields)
    }

    /// SSH channel exec (pct exec for lxc, qm agent for qemu): runs on
    /// the PVE host over the configured ssh_alias. Output is capped;
    /// stderr merged. No shell interpolation on the alias itself.
    pub fn ssh_exec(
        &mut self,
        ssh_alias: &str,
        command: &str,
        timeout: std::time::Duration,
    ) -> Result<SshExecOutput, ApiError> {
        let out = std::process::Command::new("ssh")
            .arg(ssh_alias)
            .arg("--")
            .arg(command)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();
        let out = match out {
            Ok(out) => out,
            Err(e) => {
                return Err(ApiError::Connect(format!("ssh spawn failed: {e}")));
            }
        };
        let _ = timeout; // ssh inherits the agent's own ConnectTimeout
        Ok(SshExecOutput {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout)
                .chars()
                .take(4096)
                .collect(),
            stderr: String::from_utf8_lossy(&out.stderr)
                .chars()
                .take(2048)
                .collect(),
        })
    }

    /// Clone a template into a new VM. Returns the task UPID (long task —
    /// the caller waits via wait_task).
    pub fn clone_vm(
        &mut self,
        source: &GuestRef,
        new_vmid: Vmid,
        name: &str,
    ) -> Result<Option<String>, ApiError> {
        let path = format!("/{}/clone", source.api_path());
        self.post_form(
            &path,
            &[
                ("newid".to_string(), new_vmid.get().to_string()),
                ("name".to_string(), name.to_string()),
                ("full".to_string(), "1".to_string()),
            ],
        )
    }

    /// Destroy a guest, purging disks. Returns the task UPID. The daemon
    /// layer enforces stopped-state and protected_vmids before calling.
    pub fn destroy_guest(&mut self, guest: &GuestRef) -> Result<Option<String>, ApiError> {
        let path = format!("/{}", guest.api_path());
        self.delete(
            &path,
            &[
                ("purge".to_string(), "1".to_string()),
                ("destroy-unreferenced-disks".to_string(), "1".to_string()),
            ],
        )
    }

    /// Snapshot list for one guest. The `current` pseudo-entry is
    /// filtered here — it is not a snapshot.
    pub fn snapshot_list(&mut self, guest: &GuestRef) -> Result<Vec<SnapshotEntry>, ApiError> {
        let path = format!("/{}/snapshot", guest.api_path());
        let rows: Vec<serde_json::Value> = self.get_json(&path)?.unwrap_or_default();
        Ok(rows
            .into_iter()
            .filter(|r| r.get("name").and_then(|n| n.as_str()) != Some("current"))
            .filter_map(|r| serde_json::from_value(r).ok())
            .collect())
    }

    /// Create a snapshot. Returns the task UPID (long task for large disks).
    pub fn snapshot_create(
        &mut self,
        guest: &GuestRef,
        name: &SnapshotName,
        description: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        let path = format!("/{}/snapshot", guest.api_path());
        let mut fields = vec![("snapname".to_string(), name.as_str().to_string())];
        if let Some(desc) = description {
            fields.push(("description".to_string(), desc.to_string()));
        }
        self.post_form(&path, &fields)
    }

    /// Roll back to a snapshot. Returns the task UPID. PVE stops a running
    /// guest first when the snapshot carries vmstate; without vmstate the
    /// rollback requires the guest stopped (daemon surfaces the API error).
    pub fn snapshot_rollback(
        &mut self,
        guest: &GuestRef,
        name: &SnapshotName,
    ) -> Result<Option<String>, ApiError> {
        let path = format!("/{}/snapshot/{}/rollback", guest.api_path(), name.as_str());
        self.post_form(&path, &[])
    }

    /// Delete a snapshot. Returns the task UPID.
    pub fn snapshot_delete(
        &mut self,
        guest: &GuestRef,
        name: &SnapshotName,
    ) -> Result<Option<String>, ApiError> {
        let path = format!("/{}/snapshot/{}", guest.api_path(), name.as_str());
        self.delete(&path, &[])
    }

    // -----------------------------------------------------------------------
    // Metrics / QGA read surface
    // -----------------------------------------------------------------------

    /// RRD metrics for one guest. `timeframe` is validated by the caller
    /// (daemon layer); the API accepts hour|day|week|month|year.
    pub fn guest_rrddata(
        &mut self,
        guest: &GuestRef,
        timeframe: &str,
    ) -> Result<Vec<serde_json::Value>, ApiError> {
        let path = format!(
            "/{}/rrddata?timeframe={}&cf=AVERAGE",
            guest.api_path(),
            urlencoding_encode(timeframe)
        );
        Ok(self.get_json(&path)?.unwrap_or_default())
    }

    /// Node-level RRD metrics.
    pub fn node_rrddata(
        &mut self,
        node: &NodeName,
        timeframe: &str,
    ) -> Result<Vec<serde_json::Value>, ApiError> {
        let path = format!(
            "/nodes/{}/rrddata?timeframe={}&cf=AVERAGE",
            node,
            urlencoding_encode(timeframe)
        );
        Ok(self.get_json(&path)?.unwrap_or_default())
    }

    /// QGA ping (guest agent reachable?). Only meaningful on a running
    /// qemu guest with agent enabled.
    pub fn agent_ping(&mut self, guest: &GuestRef) -> Result<bool, ApiError> {
        let path = format!("/{}/agent/ping", guest.api_path());
        match self.get_json::<serde_json::Value>(&path) {
            Ok(_) => Ok(true),
            Err(ApiError::Forbidden { .. }) => Ok(false),
            Err(ApiError::Status { status: 500, .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// QGA osinfo (running qemu guest with agent).
    pub fn agent_osinfo(&mut self, guest: &GuestRef) -> Result<serde_json::Value, ApiError> {
        let path = format!("/{}/agent/get-osinfo", guest.api_path());
        Ok(self.get_json(&path)?.unwrap_or(serde_json::Value::Null))
    }

    /// QGA network interfaces (IP discovery for a running guest).
    pub fn agent_interfaces(&mut self, guest: &GuestRef) -> Result<serde_json::Value, ApiError> {
        let path = format!("/{}/agent/network-get-interfaces", guest.api_path());
        Ok(self.get_json(&path)?.unwrap_or(serde_json::Value::Null))
    }

    /// Storage list (definitions + per-node usage merged by the caller).
    pub fn storage_list(&mut self) -> Result<Vec<StorageDef>, ApiError> {
        Ok(self.get_json("/storage")?.unwrap_or_default())
    }

    /// Per-node storage usage rows.
    pub fn node_storage(&mut self, node: &NodeName) -> Result<Vec<StorageUsage>, ApiError> {
        let path = format!("/nodes/{}/storage", node);
        Ok(self.get_json(&path)?.unwrap_or_default())
    }

    /// Storage content listing (ISOs, backups, templates).
    pub fn storage_content(
        &mut self,
        node: &NodeName,
        storage: &StorageName,
        content: Option<&str>,
    ) -> Result<Vec<serde_json::Value>, ApiError> {
        let mut path = format!("/nodes/{}/storage/{}/content", node, storage);
        if let Some(content) = content {
            path.push_str(&format!("?content={}", urlencoding_encode(content)));
        }
        Ok(self.get_json(&path)?.unwrap_or_default())
    }

    /// Task list for one node, most recent first (limit applied by caller).
    pub fn node_tasks(&mut self, node: &NodeName, limit: u32) -> Result<Vec<TaskRow>, ApiError> {
        let path = format!("/nodes/{}/tasks?limit={}", node, limit);
        Ok(self.get_json(&path)?.unwrap_or_default())
    }

    /// Method enum re-export for callers building raw requests.
    pub fn method_get() -> Method {
        Method::Get
    }
}

/// Power actions, exhaustive enum: a new action not matched everywhere is a
/// compile error, not a runtime fallthrough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Start,
    Shutdown,
    Stop,
    Reboot,
    Reset,
    Suspend,
    Resume,
}

impl PowerAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Shutdown => "shutdown",
            Self::Stop => "stop",
            Self::Reboot => "reboot",
            Self::Reset => "reset",
            Self::Suspend => "suspend",
            Self::Resume => "resume",
        }
    }

    /// Target state for idempotence checks; None = action has no single
    /// target state (reset/suspend/resume are relative to current state).
    pub fn target_state(self) -> Option<&'static str> {
        match self {
            Self::Start => Some("running"),
            Self::Shutdown | Self::Stop => Some("stopped"),
            Self::Reboot => Some("running"),
            Self::Reset | Self::Suspend | Self::Resume => None,
        }
    }

    /// Confirm gate. start/resume deliberately ungated (NANDI precedent:
    /// they change state without destroying anything — a gate people resent
    /// is a gate people route around).
    pub fn requires_confirm(self) -> bool {
        match self {
            Self::Start | Self::Resume => false,
            Self::Shutdown | Self::Stop | Self::Reboot | Self::Reset | Self::Suspend => true,
        }
    }
}

impl std::str::FromStr for PowerAction {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "start" => Ok(Self::Start),
            "shutdown" => Ok(Self::Shutdown),
            "stop" => Ok(Self::Stop),
            "reboot" => Ok(Self::Reboot),
            "reset" => Ok(Self::Reset),
            "suspend" => Ok(Self::Suspend),
            "resume" => Ok(Self::Resume),
            other => Err(format!(
                "unknown action {other:?} (expected start|shutdown|stop|reboot|reset|suspend|resume)"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_summary_parses_cluster_resources_row() {
        let raw = r#"{
            "vmid": 102, "name": "win-target-01", "type": "qemu",
            "status": "running", "node": "lan-pve",
            "maxmem": 4294967296, "maxdisk": 85899345920,
            "maxcpu": 2, "cpu": 0.0123, "mem": 1073741824, "uptime": 86400
        }"#;
        let guest: GuestSummary = serde_json::from_str(raw).unwrap();
        assert_eq!(guest.vmid, 102);
        assert_eq!(guest.ty, "qemu");
        assert_eq!(guest.guest_type(), Some(GuestType::Qemu));
        assert!(!guest.is_template());
        assert_eq!(guest.node.as_deref(), Some("lan-pve"));
        assert_eq!(guest.maxmem, Some(4 * 1024 * 1024 * 1024));
    }

    #[test]
    fn guest_summary_flags_template() {
        let raw = r#"{"vmid": 100, "type": "qemu", "template": 1}"#;
        let guest: GuestSummary = serde_json::from_str(raw).unwrap();
        assert!(guest.is_template());
        assert_eq!(guest.status, None); // #[serde(default)] tolerates absence
    }

    #[test]
    fn guest_summary_unknown_type_is_none_not_guess() {
        let raw = r#"{"vmid": 1, "type": "storage"}"#;
        let guest: GuestSummary = serde_json::from_str(raw).unwrap();
        assert_eq!(guest.guest_type(), None);
    }

    #[test]
    fn node_summary_parses() {
        let raw = r#"{"node": "lan-pve", "status": "online", "cpu": 0.036, "maxcpu": 32, "mem": 42949672960, "maxmem": 98784247808, "uptime": 1200000}"#;
        let node: NodeSummary = serde_json::from_str(raw).unwrap();
        assert_eq!(node.node, "lan-pve");
        assert_eq!(node.status.as_deref(), Some("online"));
        assert_eq!(node.maxcpu, Some(32));
    }

    #[test]
    fn version_info_parses() {
        let raw = r#"{"version": "9.2.21", "release": "5", "repoid": "4f6e0ac86f9e"}"#;
        let v: VersionInfo = serde_json::from_str(raw).unwrap();
        assert_eq!(v.version, "9.2.21");
    }
}
