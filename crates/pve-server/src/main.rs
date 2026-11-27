//! pve-mcp — daemon, MCP server and CLI entry point (scaffold: clap surface
//! lands with the daemon/MCP wiring; tools come from the ADR-001 tool table).

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
    config: Option<std::path::PathBuf>,

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
    /// Daemon and PVE connection status.
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Daemon | Command::Mcp | Command::Http | Command::Status => {
            anyhow::bail!("not yet implemented: {:?}", cli.command);
        }
    }
}
