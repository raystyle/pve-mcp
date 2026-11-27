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
    /// Install: copy pve-mcp (MCP stdio server) AND pve-cli (verb CLI)
    /// into the user application directory and add it to the user PATH.
    /// Replaces the old `register`. Daemon-safe; `update` is the
    /// force-overwrite form for upgrades.
    Install {
        /// Override the install directory (default: %LOCALAPPDATA%/pve-mcp).
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Update: overwrite an existing install (same as install with force).
    #[command(alias = "upgrade")]
    Update {
        /// Override the install directory (default: %LOCALAPPDATA%/pve-mcp).
        #[arg(long)]
        dir: Option<PathBuf>,
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
        Command::Install { dir } => run_install(dir, false).await,
        Command::Update { dir } => run_install(dir, true).await,
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

/// Rename with bounded backoff: Windows AV scanners (and indexers) can
/// hold a freshly written executable for a moment; NotFound stays fatal
/// (pnpm executable_link.rs swap_into_place form).
fn rename_with_retry(staged: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    const ATTEMPTS: usize = 10;
    const BACKOFF_MS: u64 = 25;
    for attempt in 1..ATTEMPTS {
        match std::fs::rename(staged, dest) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(e),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(
                BACKOFF_MS * u32::try_from(attempt).unwrap_or(1) as u64,
            )),
        }
    }
    std::fs::rename(staged, dest)
}

fn path_contains(dir: &std::path::Path) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p == dir))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// install / update — deploy into the user application directory
// ---------------------------------------------------------------------------

/// Install root (Windows): `%LOCALAPPDATA%/pve-mcp`.
#[cfg(windows)]
fn install_dir_default() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(|d| PathBuf::from(d).join("pve-mcp"))
        .unwrap_or_else(|| pve_core::config::app_data_dir().unwrap_or_else(|| PathBuf::from(".")))
}

/// Copy both binaries into the install dir. Daemon-safety: the running
/// daemon is the same exe — stop it first, copy, restart it (config and
/// port dir are untouched, so the restart is invisible to connected
/// stdio bridges beyond a one-off reconnect).
async fn run_install(dir_override: Option<PathBuf>, force: bool) -> Result<()> {
    let verb = if force { "update" } else { "install" };
    let exe = std::env::current_exe().context("resolving current exe")?;
    let dir = dir_override.unwrap_or_else(install_dir_default);
    std::fs::create_dir_all(&dir).context("creating install dir")?;

    let mcp_name = if cfg!(windows) {
        "pve-mcp.exe"
    } else {
        "pve-mcp"
    };
    let cli_name = if cfg!(windows) {
        "pve-cli.exe"
    } else {
        "pve-cli"
    };
    let mcp_dest = dir.join(mcp_name);
    let cli_dest = dir.join(cli_name);

    // 1. Daemon-safety: a running daemon locks the exe (Windows) and would
    //    keep serving stale code after a copy. Stop it if it is ours.
    let was_running = daemon::is_daemon_listening();
    if was_running {
        println!("[{verb}] stopping running daemon for a clean copy...");
        let _ = run_stop().await;
    }

    // 2. Copy both binaries (overwrite always: install into a fresh dir or
    //    update over an existing one — the verb already says intent).
    //    Atomic replace via temp+rename (pnpm executable_link.rs form):
    //    Linux ETXTBSY refuses writing a binary a live process holds, but
    //    rename() replaces it cleanly (peer-round lan-ubuntu finding);
    //    the staging name is pid+seq unique (concurrent installs never
    //    share a temp), failures clean up their own staging file, and the
    //    final rename retries with backoff (Windows AV scanners briefly
    //    hold freshly-written exes).
    use std::sync::atomic::{AtomicU64, Ordering};
    static STAGED_SEQ: AtomicU64 = AtomicU64::new(0);
    for (src, dest, label) in [
        (&exe, &mcp_dest, "pve-mcp (MCP stdio server)"),
        (&exe, &cli_dest, "pve-cli (verb CLI)"),
    ] {
        let seq = STAGED_SEQ.fetch_add(1, Ordering::Relaxed);
        let file_name = dest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "binary".to_string());
        let tmp = dest.with_file_name(format!(".{file_name}.{seq}.staged"));
        let copy_result = std::fs::copy(src, &tmp).and_then(|_| rename_with_retry(&tmp, dest));
        if let Err(err) = copy_result {
            let _ = std::fs::remove_file(&tmp);
            return Err(anyhow::Error::from(err))
                .with_context(|| format!("installing {}", dest.display()));
        }
        println!("[{verb}] {label}: {}", dest.display());
    }

    // 3. PATH: add to the user PATH when missing (persisted, no admin).
    if !path_contains(&dir) {
        add_to_user_path(&dir)?;
        println!(
            "[{verb}] added {} to the user PATH (new shells pick it up)",
            dir.display()
        );
    } else {
        println!("[{verb}] {} already on PATH", dir.display());
    }

    // 4. Self-check both copies, then bring the daemon back if it was up.
    for (dest, label) in [(&mcp_dest, "pve-mcp"), (&cli_dest, "pve-cli")] {
        let status = std::process::Command::new(dest)
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match status {
            Ok(s) if s.success() => println!("[{verb}] self-check ok: {label}"),
            other => {
                let _ = std::fs::remove_file(dest);
                bail!("{label} copy failed self-check: {other:?} — removed");
            }
        }
    }
    if was_running {
        println!("[{verb}] restarting daemon...");
        daemon::ensure_daemon(None)?;
        println!("[{verb}] daemon restarted on the new build");
    }
    println!("[{verb}] done — version {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

/// Append one directory to the USER path in the registry (HKCU
/// Environment\Path) and broadcast WM_SETTINGCHANGE so new shells see it
/// without a re-login. Existing entries are never touched.
#[cfg(windows)]
fn add_to_user_path(dir: &std::path::Path) -> Result<()> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_EXPAND_SZ};
    use winreg::{RegKey, RegValue};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_SET_VALUE)?;
    // Only ERROR_FILE_NOT_FOUND means "no PATH yet"; any other failure
    // (corrupt data, wrong type) must propagate — writing back a
    // default-empty value would WIPE the user's PATH (vite-plus review).
    let existing: String = match env.get_value("Path") {
        Ok(val) => val,
        Err(e) if e.raw_os_error() == Some(2) => String::new(),
        Err(e) => return Err(e.into()),
    };
    let dir_norm = dir.to_string_lossy().trim_end_matches('\\').to_string();
    let already = existing
        .split(';')
        .any(|entry| entry.trim_end_matches('\\').eq_ignore_ascii_case(&dir_norm));
    if already {
        return Ok(());
    }
    let next = if existing.is_empty() {
        dir_norm.clone()
    } else {
        format!("{dir_norm};{existing}")
    };
    // REG_EXPAND_SZ keeps %VARIABLE% entries expandable (vite-plus form).
    env.set_raw_value(
        "Path",
        &RegValue {
            vtype: REG_EXPAND_SZ,
            bytes: next
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(|c| c.to_le_bytes())
                .collect(),
        },
    )?;
    // Broadcast so Explorer/new terminals notice without re-login.
    const HWND_BROADCAST: usize = 0xFFFF;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    #[allow(unsafe_code)]
    // SAFETY: SendMessageTimeoutW with a registered-string lParam; read-only
    // broadcast, no handle ownership involved.
    unsafe {
        let mut result: usize = 0;
        windows_sys::Win32::UI::WindowsAndMessaging::SendMessageTimeoutW(
            HWND_BROADCAST as _,
            WM_SETTINGCHANGE,
            0,
            c"Environment".as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            2000,
            &mut result,
        );
    }
    Ok(())
}

/// Unix install root: `~/.local/bin` is the de-facto user bin dir
/// (XDG; already on PATH for most distros).
#[cfg(not(windows))]
fn install_dir_default() -> PathBuf {
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".local/bin"))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Unix PATH helper: `~/.local/bin` is conventionally already on PATH;
/// otherwise print the export line (shell-specific persistence is the
/// user's domain — we never edit rc files).
#[cfg(not(windows))]
fn add_to_user_path(dir: &std::path::Path) -> Result<()> {
    let on_path = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| p == dir))
        .unwrap_or(false);
    if on_path {
        return Ok(());
    }
    println!(
        "note: add this to your shell profile: export PATH=\"{}:$PATH\"",
        dir.display()
    );
    Ok(())
}
