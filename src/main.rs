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
mod input;
mod render;
mod servers;

use api::VpnBackend;
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
        /// Filter by country
        #[arg(short, long)]
        country: Option<String>,
        /// Maximum servers to show
        #[arg(short, long, default_value = "20")]
        limit: u32,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = config::load(&cli.config)?;

    match cli.command {
        None | Some(Commands::Open) => {
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .with_writer(std::io::stderr)
                .init();

            tracing::info!("launching kekkai GUI");
            run_gui(config)?;
        }
        Some(Commands::Connect { target }) => {
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .init();

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let (_api, cli_backend) = api::create_backends(&config.connection.nordvpn_path);
                let connect_target = target
                    .as_deref()
                    .or(config.connection.preferred_country.as_deref());

                let mut mgr = connection::ConnectionManager::new(cli_backend);

                tracing::info!("connecting to: {:?}", connect_target.unwrap_or("best"));
                match mgr.connect(connect_target).await {
                    Ok(()) => {
                        if let Some(info) = mgr.state().info() {
                            println!(
                                "Connected to {} ({}, {}) via {} — IP: {}",
                                info.server, info.country, info.city, info.protocol, info.ip
                            );
                        } else {
                            println!("Connected (status unavailable)");
                        }
                    }
                    Err(e) => eprintln!("Connection failed: {e}"),
                }

                Ok::<(), anyhow::Error>(())
            })?;
        }
        Some(Commands::Disconnect) => {
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .init();

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let (_api, cli_backend) = api::create_backends(&config.connection.nordvpn_path);
                let mut mgr = connection::ConnectionManager::new(cli_backend);

                // First refresh to get current state
                let _ = mgr.refresh().await;

                match mgr.disconnect().await {
                    Ok(()) => println!("Disconnected"),
                    Err(e) => eprintln!("Disconnect failed: {e}"),
                }

                Ok::<(), anyhow::Error>(())
            })?;
        }
        Some(Commands::Status) => {
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .init();

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let (_api, cli_backend) = api::create_backends(&config.connection.nordvpn_path);
                let status = cli_backend.status().await?;

                if status.connected {
                    println!("Status: Connected");
                    if let Some(ref server) = status.server {
                        println!("  Server:   {server}");
                    }
                    if let Some(ref country) = status.country {
                        println!("  Country:  {country}");
                    }
                    if let Some(ref city) = status.city {
                        println!("  City:     {city}");
                    }
                    if let Some(ref ip) = status.ip {
                        println!("  IP:       {ip}");
                    }
                    if let Some(ref protocol) = status.protocol {
                        println!("  Protocol: {protocol}");
                    }
                    if let Some(ref uptime) = status.uptime {
                        println!("  Uptime:   {uptime}");
                    }
                } else {
                    println!("Status: Disconnected");
                }

                Ok::<(), anyhow::Error>(())
            })?;
        }
        Some(Commands::Servers { country, limit }) => {
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .init();

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let (api_backend, _cli) = api::create_backends(&config.connection.nordvpn_path);
                let all_servers = api_backend.list_servers(limit, None).await?;

                let filter = servers::ServerFilter {
                    country,
                    ..Default::default()
                };
                let filtered = servers::filter_servers(&all_servers, &filter);

                println!("Servers ({} found):", filtered.len());
                for server in &filtered {
                    let load_color = match server.load {
                        0..=29 => "low",
                        30..=59 => "med",
                        60..=79 => "high",
                        _ => "full",
                    };
                    println!(
                        "  {} ({}, {}) — {}% load [{load_color}]",
                        server.hostname, server.country, server.city, server.load
                    );
                }

                if let Some(best) = servers::best_server(&all_servers, &filter) {
                    println!("\nRecommended: {best}");
                }

                Ok::<(), anyhow::Error>(())
            })?;
        }
    }

    Ok(())
}

/// Launch the GPU-rendered VPN client.
fn run_gui(config: config::KekkaiConfig) -> anyhow::Result<()> {
    use input::Action;
    use madori::{App, AppConfig, AppEvent};
    use render::KekkaiRenderer;

    let mut renderer = KekkaiRenderer::new(&config.appearance);

    // Set favorites and protocol from config
    renderer.state.favorites = config.favorites.clone();
    renderer.state.protocol = config.connection.protocol.clone();

    let app_config = AppConfig {
        title: String::from("Kekkai — NordVPN Client"),
        width: config.appearance.width,
        height: config.appearance.height,
        resizable: true,
        vsync: true,
        transparent: false,
    };

    App::builder(renderer)
        .config(app_config)
        .on_event(move |event, renderer: &mut KekkaiRenderer| {
            match event {
                AppEvent::Key(key_event) => {
                    let action = input::map_key(
                        &key_event.key,
                        key_event.pressed,
                        &key_event.modifiers,
                        &key_event.text,
                        &renderer.state.mode,
                    );

                    match action {
                        Action::Down => {
                            if renderer.state.mode == render::ViewMode::ServerMap {
                                renderer.state.map_move_down();
                            } else {
                                renderer.state.move_down();
                            }
                        }
                        Action::Up => {
                            if renderer.state.mode == render::ViewMode::ServerMap {
                                renderer.state.map_move_up();
                            } else {
                                renderer.state.move_up();
                            }
                        }
                        Action::QuickConnect => {
                            renderer.state.set_status("Quick connect: use CLI (kekkai connect)");
                        }
                        Action::Disconnect => {
                            renderer.state.set_status("Disconnect: use CLI (kekkai disconnect)");
                        }
                        Action::ShowStatus => {
                            renderer.state.mode = render::ViewMode::Status;
                        }
                        Action::SwitchToMap => {
                            renderer.state.mode = render::ViewMode::ServerMap;
                        }
                        Action::SwitchToList => {
                            renderer.state.mode = render::ViewMode::ServerList;
                        }
                        Action::ToggleFavorite => {
                            renderer.state.toggle_favorite();
                        }
                        Action::ToggleFavoritesOnly => {
                            renderer.state.favorites_only = !renderer.state.favorites_only;
                            renderer.state.refresh_display();
                            let label = if renderer.state.favorites_only {
                                "Showing favorites only"
                            } else {
                                "Showing all servers"
                            };
                            renderer.state.set_status(label);
                        }
                        Action::EnterSearch => {
                            renderer.state.enter_search();
                        }
                        Action::ConnectSelected => {
                            if let Some(server) = renderer.state.selected_server() {
                                renderer.state.set_status(
                                    format!("Connect to {}: use CLI (kekkai connect {})",
                                        server.hostname, server.hostname)
                                );
                            }
                        }
                        Action::CycleSort => {
                            renderer.state.sort_mode = renderer.state.sort_mode.next();
                            renderer.state.refresh_display();
                            renderer.state.set_status(
                                format!("Sort: {}", renderer.state.sort_mode.label())
                            );
                        }
                        Action::CycleProtocol => {
                            renderer.state.cycle_protocol();
                            renderer.state.set_status(
                                format!("Protocol: {}", renderer.state.protocol)
                            );
                        }
                        Action::Back => {
                            renderer.state.go_back();
                        }
                        Action::Quit => {
                            return madori::EventResponse {
                                consumed: true,
                                exit: true,
                                set_title: None,
                            };
                        }
                        Action::SearchInput(c) => {
                            renderer.state.search_input.insert_char(c);
                            renderer.state.apply_search();
                        }
                        Action::SearchBackspace => {
                            renderer.state.search_input.delete_back();
                            renderer.state.apply_search();
                        }
                        Action::SearchSubmit => {
                            if let Some(server) = renderer.state.selected_server() {
                                renderer.state.set_status(
                                    format!("Selected: {}", server.hostname)
                                );
                            }
                            renderer.state.go_back();
                        }
                        Action::NextView => {
                            renderer.state.mode = match renderer.state.mode {
                                render::ViewMode::Status => render::ViewMode::ServerList,
                                render::ViewMode::ServerList => render::ViewMode::ServerMap,
                                render::ViewMode::ServerMap => render::ViewMode::Status,
                                render::ViewMode::Search => render::ViewMode::ServerList,
                            };
                        }
                        Action::None => {}
                    }
                }
                AppEvent::CloseRequested => {
                    return madori::EventResponse {
                        consumed: false,
                        exit: true,
                        set_title: None,
                    };
                }
                _ => {}
            }
            madori::EventResponse::default()
        })
        .run()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(())
}
