//! NordVPN API integration.
//!
//! Two backends:
//! - NordVPN REST API (server list, recommendations, account info)
//! - `nordvpn` CLI wrapper (connect, disconnect, status, settings)
//!
//! Both implement [`VpnBackend`] for a unified interface.

use crate::servers::Server;
use serde::Deserialize;
use std::process::Command;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("NordVPN API error ({status}): {body}")]
    Api { status: u16, body: String },

    #[error("`nordvpn` CLI error: {0}")]
    Cli(String),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("not connected")]
    NotConnected,
}

pub type Result<T> = std::result::Result<T, ApiError>;

// ---------------------------------------------------------------------------
// VPN connection status returned by the CLI
// ---------------------------------------------------------------------------

/// Parsed output of `nordvpn status`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VpnStatus {
    pub connected: bool,
    pub server: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub ip: Option<String>,
    pub protocol: Option<String>,
    pub uptime: Option<String>,
}

// ---------------------------------------------------------------------------
// Backend trait
// ---------------------------------------------------------------------------

/// Common interface for NordVPN backends.
///
/// [`NordApi`] handles data fetching (server lists, recommendations).
/// [`NordCli`] handles connection lifecycle (connect, disconnect, status).
pub trait VpnBackend: Send + Sync {
    /// Fetch the full server list, optionally filtered by technology.
    fn list_servers(
        &self,
        limit: u32,
        technology: Option<&str>,
    ) -> impl std::future::Future<Output = Result<Vec<Server>>> + Send;

    /// Fetch recommended servers, optionally filtered by country ID.
    fn recommendations(
        &self,
        limit: u32,
        country_id: Option<u32>,
    ) -> impl std::future::Future<Output = Result<Vec<Server>>> + Send;

    /// Connect to a server (by hostname or country).
    fn connect(
        &self,
        target: Option<&str>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Disconnect from the VPN.
    fn disconnect(&self) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Query current connection status.
    fn status(&self) -> impl std::future::Future<Output = Result<VpnStatus>> + Send;
}

// ---------------------------------------------------------------------------
// NordVPN REST API backend (server list + recommendations)
// ---------------------------------------------------------------------------

const NORD_API_BASE: &str = "https://api.nordvpn.com/v1";

/// REST API client for NordVPN server data.
pub struct NordApi {
    client: reqwest::Client,
}

impl NordApi {
    /// Create a new API client with sensible defaults.
    #[must_use]
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client");
        Self { client }
    }

    /// Internal helper for GET requests.
    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        tracing::debug!(url, "GET");
        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiError::Api { status, body });
        }

        Ok(resp.json().await?)
    }
}

impl Default for NordApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Raw server JSON returned by the NordVPN API.
#[derive(Debug, Deserialize)]
struct ApiServer {
    id: u32,
    name: String,
    hostname: String,
    load: u8,
    #[serde(default)]
    technologies: Vec<ApiTechnology>,
    station: Option<String>,
    locations: Option<Vec<ApiLocation>>,
}

#[derive(Debug, Deserialize)]
struct ApiTechnology {
    identifier: String,
}

#[derive(Debug, Deserialize)]
struct ApiLocation {
    country: Option<ApiCountry>,
}

#[derive(Debug, Deserialize)]
struct ApiCountry {
    name: String,
    city: Option<ApiCity>,
}

#[derive(Debug, Deserialize)]
struct ApiCity {
    name: String,
}

impl From<ApiServer> for Server {
    fn from(s: ApiServer) -> Self {
        let (country, city) = s
            .locations
            .as_ref()
            .and_then(|locs| locs.first())
            .and_then(|loc| loc.country.as_ref())
            .map(|c| {
                (
                    c.name.clone(),
                    c.city.as_ref().map(|ci| ci.name.clone()).unwrap_or_default(),
                )
            })
            .unwrap_or_default();

        let technologies: Vec<String> = s.technologies.into_iter().map(|t| t.identifier).collect();
        let ip = s.station.unwrap_or_default();

        Server {
            id: s.id,
            name: s.name,
            hostname: s.hostname,
            country,
            city,
            load: s.load,
            technologies,
            ip,
        }
    }
}

impl VpnBackend for NordApi {
    async fn list_servers(
        &self,
        limit: u32,
        technology: Option<&str>,
    ) -> Result<Vec<Server>> {
        let mut url = format!("{NORD_API_BASE}/servers?limit={limit}");
        if let Some(tech) = technology {
            url.push_str(&format!(
                "&filters[servers_technologies][identifier]={tech}"
            ));
        }
        let raw: Vec<ApiServer> = self.get_json(&url).await?;
        Ok(raw.into_iter().map(Server::from).collect())
    }

    async fn recommendations(
        &self,
        limit: u32,
        country_id: Option<u32>,
    ) -> Result<Vec<Server>> {
        let mut url = format!("{NORD_API_BASE}/servers/recommendations?limit={limit}");
        if let Some(cid) = country_id {
            url.push_str(&format!("&filters[country_id]={cid}"));
        }
        let raw: Vec<ApiServer> = self.get_json(&url).await?;
        Ok(raw.into_iter().map(Server::from).collect())
    }

    /// Not supported via REST API — returns CLI error.
    async fn connect(&self, _target: Option<&str>) -> Result<()> {
        Err(ApiError::Cli(
            "connect is only supported via the nordvpn CLI backend".into(),
        ))
    }

    /// Not supported via REST API — returns CLI error.
    async fn disconnect(&self) -> Result<()> {
        Err(ApiError::Cli(
            "disconnect is only supported via the nordvpn CLI backend".into(),
        ))
    }

    /// Not supported via REST API — returns CLI error.
    async fn status(&self) -> Result<VpnStatus> {
        Err(ApiError::Cli(
            "status is only supported via the nordvpn CLI backend".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// `nordvpn` CLI backend (connect, disconnect, status)
// ---------------------------------------------------------------------------

/// Wraps the `nordvpn` CLI binary for connection lifecycle operations.
pub struct NordCli {
    nordvpn_path: String,
}

impl NordCli {
    #[must_use]
    pub fn new(nordvpn_path: &str) -> Self {
        Self {
            nordvpn_path: nordvpn_path.to_string(),
        }
    }

    /// Run a `nordvpn` CLI command and return stdout.
    fn run(&self, args: &[&str]) -> Result<String> {
        tracing::debug!(path = %self.nordvpn_path, ?args, "running nordvpn CLI");

        let output = Command::new(&self.nordvpn_path)
            .args(args)
            .output()
            .map_err(|e| ApiError::Cli(format!("failed to run `nordvpn`: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ApiError::Cli(stderr.into_owned()));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| ApiError::Cli(format!("invalid UTF-8 output: {e}")))
    }

    /// Parse the output of `nordvpn status` into a [`VpnStatus`].
    fn parse_status(output: &str) -> VpnStatus {
        let mut status = VpnStatus {
            connected: false,
            server: None,
            country: None,
            city: None,
            ip: None,
            protocol: None,
            uptime: None,
        };

        for line in output.lines() {
            let line = line.trim();
            if line.contains("Status: Connected") || line.contains("Status: connected") {
                status.connected = true;
            }
            if let Some(val) = Self::extract_field(line, "Current server:") {
                status.server = Some(val);
            }
            if let Some(val) = Self::extract_field(line, "Country:") {
                status.country = Some(val);
            }
            if let Some(val) = Self::extract_field(line, "City:") {
                status.city = Some(val);
            }
            if let Some(val) = Self::extract_field(line, "Server IP:") {
                status.ip = Some(val);
            }
            if let Some(val) = Self::extract_field(line, "Current protocol:") {
                status.protocol = Some(val);
            }
            if let Some(val) = Self::extract_field(line, "Uptime:") {
                status.uptime = Some(val);
            }
        }

        status
    }

    fn extract_field(line: &str, prefix: &str) -> Option<String> {
        if line.starts_with(prefix) {
            Some(line[prefix.len()..].trim().to_string())
        } else {
            None
        }
    }
}

impl VpnBackend for NordCli {
    /// Not supported via CLI — use [`NordApi`] for server lists.
    async fn list_servers(
        &self,
        _limit: u32,
        _technology: Option<&str>,
    ) -> Result<Vec<Server>> {
        Err(ApiError::Cli(
            "list_servers is only supported via the NordVPN REST API backend".into(),
        ))
    }

    /// Not supported via CLI — use [`NordApi`] for recommendations.
    async fn recommendations(
        &self,
        _limit: u32,
        _country_id: Option<u32>,
    ) -> Result<Vec<Server>> {
        Err(ApiError::Cli(
            "recommendations is only supported via the NordVPN REST API backend".into(),
        ))
    }

    async fn connect(&self, target: Option<&str>) -> Result<()> {
        let mut args = vec!["connect"];
        if let Some(t) = target {
            args.push(t);
        }
        let output = self.run(&args)?;
        tracing::info!(%output, "nordvpn connect");
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        let output = self.run(&["disconnect"])?;
        tracing::info!(%output, "nordvpn disconnect");
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus> {
        let output = self.run(&["status"])?;
        Ok(Self::parse_status(&output))
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Create the appropriate backend pair from configuration.
///
/// Returns `(api_backend, cli_backend)` — use the API backend for server data
/// and the CLI backend for connection operations.
#[must_use]
pub fn create_backends(nordvpn_path: &str) -> (NordApi, NordCli) {
    tracing::info!("creating NordVPN backends (API + CLI at {nordvpn_path})");
    (NordApi::new(), NordCli::new(nordvpn_path))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_status_connected() {
        let output = "\
Status: Connected
Current server: us1234.nordvpn.com
Country: United States
City: New York
Server IP: 203.0.113.42
Current protocol: NordLynx
Uptime: 2 hours 15 minutes";

        let status = NordCli::parse_status(output);
        assert!(status.connected);
        assert_eq!(status.server.as_deref(), Some("us1234.nordvpn.com"));
        assert_eq!(status.country.as_deref(), Some("United States"));
        assert_eq!(status.city.as_deref(), Some("New York"));
        assert_eq!(status.ip.as_deref(), Some("203.0.113.42"));
        assert_eq!(status.protocol.as_deref(), Some("NordLynx"));
        assert_eq!(status.uptime.as_deref(), Some("2 hours 15 minutes"));
    }

    #[test]
    fn parse_status_disconnected() {
        let output = "\
Status: Disconnected";

        let status = NordCli::parse_status(output);
        assert!(!status.connected);
        assert!(status.server.is_none());
        assert!(status.ip.is_none());
    }

    #[test]
    fn api_server_to_server() {
        let raw = ApiServer {
            id: 42,
            name: "United States #1234".into(),
            hostname: "us1234.nordvpn.com".into(),
            load: 30,
            technologies: vec![
                ApiTechnology {
                    identifier: "wireguard_udp".into(),
                },
                ApiTechnology {
                    identifier: "openvpn_tcp".into(),
                },
            ],
            station: Some("203.0.113.42".into()),
            locations: Some(vec![ApiLocation {
                country: Some(ApiCountry {
                    name: "United States".into(),
                    city: Some(ApiCity {
                        name: "New York".into(),
                    }),
                }),
            }]),
        };

        let server = Server::from(raw);
        assert_eq!(server.id, 42);
        assert_eq!(server.hostname, "us1234.nordvpn.com");
        assert_eq!(server.country, "United States");
        assert_eq!(server.city, "New York");
        assert_eq!(server.load, 30);
        assert_eq!(server.ip, "203.0.113.42");
        assert!(server.technologies.contains(&"wireguard_udp".to_string()));
    }

    #[test]
    fn api_server_missing_location() {
        let raw = ApiServer {
            id: 1,
            name: "Unknown".into(),
            hostname: "xx1.nordvpn.com".into(),
            load: 0,
            technologies: vec![],
            station: None,
            locations: None,
        };

        let server = Server::from(raw);
        assert_eq!(server.country, "");
        assert_eq!(server.city, "");
        assert_eq!(server.ip, "");
    }
}
