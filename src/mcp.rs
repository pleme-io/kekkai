//! MCP server for kekkai VPN client automation.
//!
//! Tools:
//!   `status`              — current VPN connection status
//!   `version`             — server version info
//!   `config_get`          — get a config value by key
//!   `config_set`          — set a config value
//!   `connect`             — connect to a VPN server
//!   `disconnect`          — disconnect from VPN
//!   `server_list`         — list available VPN servers
//!   `connection_status`   — detailed connection info (IP, protocol, uptime, transfer)
//!   `set_protocol`        — change VPN protocol (NordLynx/OpenVPN)
//!   `list_favorites`      — list favorite/pinned servers

use kaname::rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use serde_json::json;

// ── Tool input types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConfigGetInput {
    #[schemars(description = "Config key to retrieve (e.g. 'connection.protocol', 'api.cache_ttl_secs').")]
    key: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConfigSetInput {
    #[schemars(description = "Config key to set.")]
    key: String,
    #[schemars(description = "New value as a JSON string.")]
    value: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConnectInput {
    #[schemars(description = "Country code or server name to connect to (e.g. 'us', 'de', 'jp'). Omit for recommended server.")]
    country: Option<String>,
    #[schemars(description = "City name within the country (e.g. 'new_york', 'tokyo').")]
    city: Option<String>,
    #[schemars(description = "Protocol to use: 'nordlynx', 'openvpn_udp', or 'openvpn_tcp'. Uses config default if omitted.")]
    protocol: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ServerListInput {
    #[schemars(description = "Filter by country name or code.")]
    country: Option<String>,
    #[schemars(description = "Filter by protocol support: 'nordlynx' or 'openvpn'.")]
    protocol: Option<String>,
    #[schemars(description = "Maximum server load percentage (0-100). Only show servers below this load.")]
    max_load: Option<u32>,
    #[schemars(description = "Maximum number of servers to return (default 20).")]
    limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SetProtocolInput {
    #[schemars(description = "VPN protocol: 'nordlynx', 'openvpn_udp', or 'openvpn_tcp'.")]
    protocol: String,
}

// ── MCP Server ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct KekkaiMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl KekkaiMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    // ── Standard tools ──────────────────────────────────────────────────────

    #[tool(description = "Get current VPN connection status: connected/disconnected, server, IP, protocol.")]
    async fn status(&self) -> String {
        // TODO: query nordvpn CLI status
        serde_json::to_string(&json!({
            "connected": false,
            "server": null,
            "country": null,
            "city": null,
            "ip": null,
            "protocol": null
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Get kekkai version information.")]
    async fn version(&self) -> String {
        serde_json::to_string(&json!({
            "name": "kekkai",
            "crate": "mamorigami",
            "version": env!("CARGO_PKG_VERSION"),
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Get a configuration value by key.")]
    async fn config_get(&self, Parameters(input): Parameters<ConfigGetInput>) -> String {
        // TODO: read from shikumi ConfigStore
        serde_json::to_string(&json!({
            "key": input.key,
            "value": null,
            "error": "config store not connected"
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Set a configuration value. Changes are applied immediately via hot-reload.")]
    async fn config_set(&self, Parameters(input): Parameters<ConfigSetInput>) -> String {
        // TODO: write to shikumi ConfigStore
        serde_json::to_string(&json!({
            "key": input.key,
            "value": input.value,
            "applied": false,
            "error": "config store not connected"
        }))
        .unwrap_or_default()
    }

    // ── VPN tools ───────────────────────────────────────────────────────────

    #[tool(description = "Connect to a VPN server. Optionally specify country, city, or protocol. Uses recommended server if no target given.")]
    async fn connect(&self, Parameters(input): Parameters<ConnectInput>) -> String {
        // TODO: invoke nordvpn CLI connect
        serde_json::to_string(&json!({
            "ok": false,
            "country": input.country,
            "city": input.city,
            "protocol": input.protocol,
            "error": "nordvpn CLI not available"
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Disconnect from the current VPN server.")]
    async fn disconnect(&self) -> String {
        // TODO: invoke nordvpn CLI disconnect
        serde_json::to_string(&json!({
            "ok": false,
            "error": "nordvpn CLI not available"
        }))
        .unwrap_or_default()
    }

    #[tool(description = "List available VPN servers. Filter by country, protocol, or load.")]
    async fn server_list(&self, Parameters(input): Parameters<ServerListInput>) -> String {
        let limit = input.limit.unwrap_or(20);
        // TODO: query NordVPN REST API
        serde_json::to_string(&json!({
            "country_filter": input.country,
            "protocol_filter": input.protocol,
            "max_load": input.max_load,
            "limit": limit,
            "servers": [],
            "total": 0
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Get detailed connection status: server, IP, protocol, uptime, and data transfer statistics.")]
    async fn connection_status(&self) -> String {
        // TODO: query nordvpn CLI status
        serde_json::to_string(&json!({
            "connected": false,
            "server": null,
            "ip": null,
            "protocol": null,
            "uptime": null,
            "transfer": {
                "received_bytes": 0,
                "sent_bytes": 0
            }
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Change the VPN protocol. Options: 'nordlynx' (WireGuard), 'openvpn_udp', 'openvpn_tcp'.")]
    async fn set_protocol(&self, Parameters(input): Parameters<SetProtocolInput>) -> String {
        // TODO: invoke nordvpn CLI set protocol
        serde_json::to_string(&json!({
            "ok": false,
            "protocol": input.protocol,
            "error": "nordvpn CLI not available"
        }))
        .unwrap_or_default()
    }

    #[tool(description = "List favorite/pinned VPN servers from configuration.")]
    async fn list_favorites(&self) -> String {
        // TODO: read favorites from shikumi config
        serde_json::to_string(&json!({
            "favorites": [],
            "total": 0
        }))
        .unwrap_or_default()
    }
}

#[tool_handler]
impl ServerHandler for KekkaiMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Kekkai NordVPN client — connect/disconnect, server selection, protocol management."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let server = KekkaiMcp::new().serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
