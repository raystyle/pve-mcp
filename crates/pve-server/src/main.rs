//! pve-mcp — daemon, MCP server, CLI entry point and `pve-cli` verb host.
//!
//! Three consumption forms (know-mcp heritage): MCP stdio (Claude Code),
//! MCP HTTP (later), and CLI verbs. The verb form runs through a copied
//! binary named `pve-cli` (see `register`): the exe file stem is the
//! dispatch key — same native self-registration approach mcp2cli used for
//! Windows, internalized. Verbs mirror the MCP tools 1:1 and talk to the
//! daemon admin plane directly, no MCP protocol layer.

mod client_bridge;
mod daemon;
mod mcp;

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "pve-mcp",
    version,
    about = "Proxmox VE virtualization MCP server"
)]
struct Cli {
    /// Path to pve-mcp.toml (default: PVE_MCP_CONFIG env or exe-adjacent).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Admin port-file directory (default: PVE_MCP_PORT_DIR or %APPDATA%/pve-mcp).
    /// Echoed through spawns, which do not inherit the environment.
    #[arg(long, global = true)]
    port_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the background daemon explicitly (normally auto-spawned).
    Daemon,
    /// Internal: ensure_daemon spawns  (works from both
    /// pve-mcp.exe and the pve-cli copy).
    #[command(name = "daemon-run", hide = true)]
    DaemonRun,
    /// MCP over stdio (Claude Code entry point; bridges to the daemon).
    #[command(name = "stdio")]
    Mcp,
    /// MCP over streamable HTTP (served by the daemon).
    Http,
    /// Daemon and PVE connection status (starts the daemon if needed).
    Status,
    /// Stop the running daemon.
    Stop,
    /// Register `pve-cli`: copy this binary under the CLI name. Re-run
    /// with --force after every pve-mcp upgrade (know-mcp heritage).
    #[command(name = "register")]
    Register {
        /// Directory to place pve-cli in (default: exe-adjacent).
        #[arg(long)]
        install_dir: Option<PathBuf>,
        /// Overwrite an existing pve-cli copy.
        #[arg(long)]
        force: bool,
    },
}

/// Verb CLI invoked via the `pve-cli` binary name: the register command
/// copies this binary under that name, and the exe stem is the dispatch
/// key. Verbs mirror the MCP tools 1:1 over the daemon admin plane.
#[derive(Parser)]
#[command(name = "pve-cli", about = "pve-mcp CLI verbs (MCP tools 1:1)")]
struct PveCliArgs {
    /// Path to pve-mcp.toml.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Admin port-file directory.
    #[arg(long, global = true)]
    port_dir: Option<PathBuf>,

    /// Machine-readable envelope: {"app":"pve-cli","verb":...,"data":...}.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: PveCliVerb,
}

#[derive(clap::Subcommand)]
pub enum PveCliVerb {
    /// Cluster-level resources (the cluster_status / vm_list tools).
    Cluster {
        #[command(subcommand)]
        command: ClusterCommand,
    },
    /// Guest (VM/container) operations — the vm_* MCP tools.
    Vm {
        #[command(subcommand)]
        command: VmCommand,
    },
    /// Daemon control.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
    /// Internal: daemon runtime — ensure_daemon spawns `<current-exe> daemon`,
    /// and under pve-cli that exe IS pve-cli.exe (know-mcp heritage).
    #[command(hide = true)]
    DaemonRun,
}

#[derive(clap::Subcommand)]
pub enum ClusterCommand {
    /// Cluster overview: version, nodes, guest counts.
    Status,
    /// List guests (QEMU + LXC); filters narrow the table.
    List {
        /// Filter qemu | lxc.
        #[arg(long)]
        ty: Option<String>,
        /// Filter running | stopped.
        #[arg(long)]
        status: Option<String>,
    },
}

#[derive(clap::Subcommand)]
pub enum VmCommand {
    /// Power on (ungated: changes state without destroying anything).
    Start { vmid: u32 },
    /// Graceful ACPI shutdown (gated: needs --confirm).
    Shutdown {
        vmid: u32,
        #[arg(long)]
        confirm: bool,
    },
    /// Hard power cut (gated: needs --confirm; emergency use).
    Stop {
        vmid: u32,
        #[arg(long)]
        confirm: bool,
    },
    /// Warm reboot (gated).
    Reboot {
        vmid: u32,
        #[arg(long)]
        confirm: bool,
    },
    /// Hard reset button (gated).
    Reset {
        vmid: u32,
        #[arg(long)]
        confirm: bool,
    },
    /// Pause to memory (gated).
    Suspend {
        vmid: u32,
        #[arg(long)]
        confirm: bool,
    },
    /// Unpause (ungated).
    Resume { vmid: u32 },
    /// One guest's detail: runtime + config.
    Status { vmid: u32 },
    /// Clone a template into a new VM (dry-run until --confirm).
    Create {
        /// New VM name (no whitespace, no '/').
        #[arg(long)]
        name: String,
        /// Source template vmid.
        #[arg(long)]
        template: u32,
        /// Explicit new vmid (omit = cluster next free).
        #[arg(long)]
        new_vmid: Option<u32>,
        /// Execute (without: dry-run plan).
        #[arg(long)]
        confirm: bool,
    },
    /// Destroy a stopped guest, purging disks (dry-run until --confirm;
    /// running guests are refused even with --confirm).
    Delete {
        vmid: u32,
        /// Execute (without: dry-run plan).
        #[arg(long)]
        confirm: bool,
    },
}

#[derive(clap::Subcommand)]
pub enum DaemonCommand {
    /// Daemon version/pid/tool count.
    Status,
    /// Stop the daemon.
    Stop,
}

/// True when this process was launched through the `pve-cli` copy (the exe
/// file stem is the dispatch key — same routing mcp2cli used for its
/// copied aliases on Windows).
fn pve_cli_invoked() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .as_deref()
        == Some("pve-cli")
}

#[tokio::main]
async fn main() -> Result<()> {
    if pve_cli_invoked() {
        let args: PveCliArgs = PveCliArgs::parse();
        if let Some(dir) = args.port_dir.clone() {
            std::env::set_var("PVE_MCP_PORT_DIR", dir);
        }
        return run_cli_verb(args.config, args.json, args.command).await;
    }

    let cli = Cli::parse();
    if let Some(dir) = cli.port_dir.clone() {
        std::env::set_var("PVE_MCP_PORT_DIR", dir);
    }
    match cli.command {
        Command::Daemon | Command::DaemonRun => daemon::run_daemon(cli.config, None),
        Command::Status => run_status(cli.config).await,
        Command::Stop => run_stop().await,
        Command::Mcp => mcp::serve_stdio(cli.config).await,
        Command::Http => bail!("MCP HTTP surface lands with the tool table"),
        Command::Register { install_dir, force } => run_register(install_dir, force),
    }
}

// ---------------------------------------------------------------------------
// pve-cli verb runner
// ---------------------------------------------------------------------------

async fn run_status(config: Option<PathBuf>) -> Result<()> {
    daemon::ensure_daemon(config.as_deref())?;
    let response = client_bridge::admin_request("status", serde_json::json!({}))?;
    println!("{}", serde_json::to_string_pretty(&response["result"])?);
    Ok(())
}

async fn run_stop() -> Result<()> {
    if !daemon::is_daemon_listening() {
        println!("daemon not running");
        return Ok(());
    }
    let _ = client_bridge::admin_request("stop", serde_json::json!({}));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        if !daemon::is_daemon_listening() {
            println!("daemon stopped");
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    anyhow::bail!("daemon did not stop within 15s");
}

async fn run_cli_verb(config: Option<PathBuf>, json: bool, verb: PveCliVerb) -> Result<()> {
    use serde_json::json;
    type Renderer = Box<dyn FnOnce(&serde_json::Value) -> String>;
    let (method, params, render): (&str, serde_json::Value, Renderer) = match verb {
        PveCliVerb::Cluster {
            command: ClusterCommand::Status,
        } => (
            "cluster_status",
            json!({}),
            Box::new(|r| {
                format!(
                    "PVE {} | nodes {} | guests {} (running {}, qemu {}, lxc {})",
                    r["pve_version"].as_str().unwrap_or("?"),
                    r["nodes"].as_array().map(|a| a.len()).unwrap_or(0),
                    r["guests"]["total"].as_u64().unwrap_or(0),
                    r["guests"]["running"].as_u64().unwrap_or(0),
                    r["guests"]["qemu"].as_u64().unwrap_or(0),
                    r["guests"]["lxc"].as_u64().unwrap_or(0),
                )
            }),
        ),
        PveCliVerb::Cluster {
            command: ClusterCommand::List { ty, status },
        } => {
            let mut params = json!({});
            if let Some(ty) = ty {
                params["ty"] = json!(ty);
            }
            if let Some(status) = status {
                params["status"] = json!(status);
            }
            (
                "vm_list",
                params,
                Box::new(|r| {
                    let guests = r["guests"].as_array().cloned().unwrap_or_default();
                    let mut out = format!("{} guests:\n", r["count"].as_u64().unwrap_or(0));
                    for g in guests {
                        out.push_str(&format!(
                            "  {:>4}  {:<22} {:<8} {:<9} {:<8} {}\n",
                            g["vmid"].as_u64().unwrap_or(0),
                            g["name"].as_str().unwrap_or("?"),
                            g["ty"].as_str().unwrap_or("?"),
                            g["status"].as_str().unwrap_or("?"),
                            g["node"].as_str().unwrap_or("?"),
                            if g["template"].as_bool().unwrap_or(false) {
                                "[tpl]"
                            } else {
                                ""
                            },
                        ));
                    }
                    out
                }),
            )
        }
        PveCliVerb::Vm {
            command: VmCommand::Status { vmid },
        } => (
            "vm_status",
            json!({"vmid": vmid}),
            Box::new(|r| {
                format!(
                    "{} {} ({}): {} | cores {} | mem {}MB",
                    r["vmid"].as_u64().unwrap_or(0),
                    r["config"]["name"].as_str().unwrap_or("?"),
                    r["ty"].as_str().unwrap_or("?"),
                    r["current"]["status"].as_str().unwrap_or("?"),
                    r["config"]["cores"].as_u64().unwrap_or(0),
                    r["config"]["memory"]
                        .as_str()
                        .and_then(|m| m.parse::<u64>().ok())
                        .unwrap_or(r["config"]["memory"].as_u64().unwrap_or(0)),
                )
            }),
        ),
        PveCliVerb::Vm {
            command: VmCommand::Start { vmid },
        } => power_call(vmid, "start", false),
        PveCliVerb::Vm {
            command: VmCommand::Resume { vmid },
        } => power_call(vmid, "resume", false),
        PveCliVerb::Vm {
            command: VmCommand::Shutdown { vmid, confirm },
        } => power_call(vmid, "shutdown", confirm),
        PveCliVerb::Vm {
            command: VmCommand::Stop { vmid, confirm },
        } => power_call(vmid, "stop", confirm),
        PveCliVerb::Vm {
            command: VmCommand::Reboot { vmid, confirm },
        } => power_call(vmid, "reboot", confirm),
        PveCliVerb::Vm {
            command: VmCommand::Reset { vmid, confirm },
        } => power_call(vmid, "reset", confirm),
        PveCliVerb::Vm {
            command: VmCommand::Suspend { vmid, confirm },
        } => power_call(vmid, "suspend", confirm),
        PveCliVerb::Vm {
            command:
                VmCommand::Create {
                    name,
                    template,
                    new_vmid,
                    confirm,
                },
        } => {
            let mut params = json!({"name": name, "template": template, "confirm": confirm});
            if let Some(v) = new_vmid {
                params["new_vmid"] = json!(v);
            }
            (
                "vm_create",
                params,
                Box::new(|r| {
                    if r["dry_run"].as_bool().unwrap_or(false) {
                        let plan = &r["plan"];
                        format!(
                            "dry-run: clone {} -> vmid {} named {} (pass --confirm to execute)",
                            plan["source"].as_u64().unwrap_or(0),
                            plan["new_vmid"].as_u64().unwrap_or(0),
                            plan["name"].as_str().unwrap_or("?"),
                        )
                    } else {
                        format!(
                            "created vmid {}: ok={} exit={}",
                            r["new_vmid"].as_u64().unwrap_or(0),
                            r["ok"].as_bool().unwrap_or(false),
                            r["exitstatus"].as_str().unwrap_or("?"),
                        )
                    }
                }),
            )
        }
        PveCliVerb::Vm {
            command: VmCommand::Delete { vmid, confirm },
        } => (
            "vm_delete",
            json!({"vmid": vmid, "confirm": confirm}),
            Box::new(|r| {
                if r["dry_run"].as_bool().unwrap_or(false) {
                    let plan = &r["plan"];
                    format!(
                        "dry-run: destroy vmid {} ({} on {}, {}) — pass --confirm to execute",
                        plan["vmid"].as_u64().unwrap_or(0),
                        plan["ty"].as_str().unwrap_or("?"),
                        plan["node"].as_str().unwrap_or("?"),
                        plan["status"].as_str().unwrap_or("?"),
                    )
                } else {
                    format!(
                        "destroyed: ok={} exit={}",
                        r["ok"].as_bool().unwrap_or(false),
                        r["exitstatus"].as_str().unwrap_or("?"),
                    )
                }
            }),
        ),
        PveCliVerb::Daemon {
            command: DaemonCommand::Status,
        } => return run_status(config).await,
        PveCliVerb::Daemon {
            command: DaemonCommand::Stop,
        } => return run_stop().await,
        PveCliVerb::DaemonRun => return daemon::run_daemon(config, None),
    };

    daemon::ensure_daemon(config.as_deref())?;
    let response = client_bridge::admin_request(method, params)?;
    if let Some(err) = response.get("error").and_then(|e| e.as_str()) {
        bail!("{err}");
    }
    let result = response["result"].clone();
    if json {
        let envelope = json!({
            "app": "pve-cli",
            "verb": method,
            "data": result,
        });
        println!("{}", serde_json::to_string(&envelope)?);
    } else {
        print!("{}", render(&result));
        if !result.to_string().ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

type VerbCall = (
    &'static str,
    serde_json::Value,
    Box<dyn FnOnce(&serde_json::Value) -> String>,
);

/// Shared power-verb wiring: every power subcommand lands here with its
/// action name and confirm flag (daemon re-checks the gate anyway).
fn power_call(vmid: u32, action: &str, confirm: bool) -> VerbCall {
    use serde_json::json;
    (
        "vm_power",
        json!({"vmid": vmid, "action": action, "confirm": confirm, "wait": true}),
        Box::new(|r| {
            if r["skipped"].as_bool().unwrap_or(false) {
                format!("skipped: {}", r["reason"].as_str().unwrap_or("?"))
            } else {
                format!(
                    "{}: ok={} exit={}",
                    r["action"].as_str().unwrap_or("?"),
                    r["ok"].as_bool().unwrap_or(false),
                    r["exitstatus"].as_str().unwrap_or("?"),
                )
            }
        }),
    )
}

fn run_register(install_dir: Option<PathBuf>, force: bool) -> Result<()> {
    let exe = std::env::current_exe().context("resolving current exe")?;
    let exe_dir = exe.parent().context("exe has no parent")?.to_path_buf();
    let dir = install_dir.unwrap_or_else(|| exe_dir.clone());
    std::fs::create_dir_all(&dir).context("creating install dir")?;
    let cli_name = if cfg!(windows) {
        "pve-cli.exe"
    } else {
        "pve-cli"
    };
    let dest = dir.join(cli_name);
    if dir != exe_dir && dest.exists() && !force {
        bail!(
            "{} already exists — pass --force to overwrite (or re-register after upgrading pve-mcp)",
            dest.display()
        );
    }
    std::fs::copy(&exe, &dest)
        .with_context(|| format!("copying {} -> {}", exe.display(), dest.display()))?;
    println!("registered: {}", dest.display());
    if !path_contains(&dir) {
        println!(
            "note: {} is not on PATH — add it to use `pve-cli` from anywhere",
            dir.display()
        );
    }
    let status = std::process::Command::new(&dest)
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => println!("self-check ok: pve-cli --help"),
        other => {
            let _ = std::fs::remove_file(&dest);
            bail!("pve-cli copy failed self-check: {other:?} — removed");
        }
    }
    println!("try: pve-cli cluster-status");
    println!("     pve-cli vm-list --ty lxc");
    Ok(())
}

fn path_contains(dir: &std::path::Path) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p == dir))
        .unwrap_or(false)
}
