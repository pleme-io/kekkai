//! Kekkai (結界) — GPU-rendered NordVPN client.
//!
//! Replaces the NordVPN GUI while using the NordVPN service:
//! - GPU-accelerated UI via garasu (wgpu/winit)
//! - NordVPN API for server list, account, connection management
//! - Server map visualization with latency indicators
//! - Quick-connect with smart server selection
//! - Hot-reloadable configuration via shikumi

mod api;
mod config;
mod connection;
mod render;
mod servers;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "kekkai", version, about = "GPU-rendered NordVPN client")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Configuration file override
    #[arg(long, env = "KEKKAI_CONFIG")]
    config: Option<std::path::PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the GUI
    Open,
    /// Quick-connect to best available server
    Connect {
        /// Country code or server name
        target: Option<String>,
    },
    /// Disconnect from VPN
    Disconnect,
    /// Show connection status
    Status,
    /// List available servers
    Servers {
        /// Filter by country code
        #[arg(short, long)]
        country: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = config::load(&cli.config)?;

    match cli.command {
        None | Some(Commands::Open) => {
            tracing::info!("launching kekkai");
            // TODO: Initialize garasu GPU context
            // TODO: Create winit window with server map UI
        }
        Some(Commands::Connect { target }) => {
            tracing::info!("connecting to: {:?}", target.as_deref().unwrap_or("best"));
            // TODO: Call nordvpn CLI or API to connect
        }
        Some(Commands::Disconnect) => {
            tracing::info!("disconnecting");
            // TODO: Call nordvpn CLI to disconnect
        }
        Some(Commands::Status) => {
            // TODO: Show connection status
        }
        Some(Commands::Servers { country }) => {
            // TODO: List servers, optionally filtered
            tracing::info!("listing servers for: {:?}", country);
        }
    }

    Ok(())
}
