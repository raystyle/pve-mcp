//! pve-mcp — daemon, MCP server and CLI entry point.

mod client_bridge;
mod daemon;
mod mcp;

use std::path::PathBuf;

use anyhow::Result;
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
    /// MCP over stdio (Claude Code entry point; bridges to the daemon).
    #[command(name = "stdio")]
    Mcp,
    /// MCP over streamable HTTP (served by the daemon).
    Http,
    /// Daemon and PVE connection status (starts the daemon if needed).
    Status,
    /// Stop the running daemon.
    Stop,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(dir) = cli.port_dir.clone() {
        std::env::set_var("PVE_MCP_PORT_DIR", dir);
    }
    match cli.command {
        Command::Daemon => daemon::run_daemon(cli.config, None),
        Command::Status => run_status(cli.config).await,
        Command::Stop => run_stop().await,
        Command::Mcp => mcp::serve_stdio(cli.config).await,
        Command::Http => anyhow::bail!("MCP HTTP surface lands with the tool table"),
    }
}

async fn run_status(config: Option<PathBuf>) -> Result<()> {
    daemon::ensure_daemon(config.as_deref())?;
    let response = daemon::admin_request("status", serde_json::json!({}))?;
    println!("{}", serde_json::to_string_pretty(&response["result"])?);
    Ok(())
}

async fn run_stop() -> Result<()> {
    if !daemon::is_daemon_listening() {
        println!("daemon not running");
        return Ok(());
    }
    let _ = daemon::admin_request("stop", serde_json::json!({}));
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
