//! Connection management — connect, disconnect, monitor status.
//!
//! Wraps `nordvpn` CLI for connection lifecycle.
//! Monitors connection state changes and reports to the UI.
//!
//! The [`ConnectionManager`] maintains a state machine that tracks
//! the VPN connection through its lifecycle:
//!
//! ```text
//! Disconnected → Connecting → Connected(info) → Disconnecting → Disconnected
//! ```

use crate::api::{self, VpnBackend};
use serde::{Deserialize, Serialize};
use std::time::Instant;

// ---------------------------------------------------------------------------
// Connection state machine
// ---------------------------------------------------------------------------

/// Current state of the VPN connection.
#[derive(Debug, Clone)]
pub enum ConnectionState {
    /// Not connected to any server.
    Disconnected,
    /// Initiating a connection to a server.
    Connecting,
    /// Connected to a server with active session info.
    Connected(ConnectionInfo),
    /// Tearing down the connection.
    Disconnecting,
}

impl ConnectionState {
    /// Returns `true` if currently connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected(_))
    }

    /// Returns `true` if in a transitional state (connecting or disconnecting).
    #[must_use]
    pub fn is_transitioning(&self) -> bool {
        matches!(self, Self::Connecting | Self::Disconnecting)
    }

    /// Returns the connection info if connected.
    #[must_use]
    pub fn info(&self) -> Option<&ConnectionInfo> {
        match self {
            Self::Connected(info) => Some(info),
            _ => None,
        }
    }

    /// Human-readable label for the current state.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::Connected(_) => "Connected",
            Self::Disconnecting => "Disconnecting",
        }
    }
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Connecting => write!(f, "Connecting..."),
            Self::Connected(info) => write!(
                f,
                "Connected to {} ({}) via {}",
                info.server, info.ip, info.protocol
            ),
            Self::Disconnecting => write!(f, "Disconnecting..."),
        }
    }
}

/// Information about an active VPN connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// Server hostname (e.g. "us1234.nordvpn.com").
    pub server: String,
    /// VPN protocol in use (e.g. "NordLynx", "OpenVPN").
    pub protocol: String,
    /// Assigned IP address.
    pub ip: String,
    /// Country of the connected server.
    pub country: String,
    /// City of the connected server.
    pub city: String,
    /// When the connection was established.
    #[serde(skip)]
    pub connected_at: Option<Instant>,
}

impl ConnectionInfo {
    /// Duration since the connection was established.
    #[must_use]
    pub fn uptime(&self) -> Option<std::time::Duration> {
        self.connected_at.map(|t| t.elapsed())
    }
}

// ---------------------------------------------------------------------------
// Connection manager
// ---------------------------------------------------------------------------

/// Manages VPN connection lifecycle using a [`VpnBackend`].
pub struct ConnectionManager<B: VpnBackend> {
    state: ConnectionState,
    backend: B,
}

impl<B: VpnBackend> ConnectionManager<B> {
    /// Create a new manager starting in the [`Disconnected`](ConnectionState::Disconnected) state.
    #[must_use]
    pub fn new(backend: B) -> Self {
        Self {
            state: ConnectionState::Disconnected,
            backend,
        }
    }

    /// Current connection state.
    #[must_use]
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }

    /// Connect to the VPN, optionally targeting a specific server or country.
    ///
    /// Transitions: `Disconnected → Connecting → Connected` (or back to `Disconnected` on failure).
    pub async fn connect(&mut self, target: Option<&str>) -> api::Result<()> {
        if self.state.is_connected() || self.state.is_transitioning() {
            tracing::warn!(state = %self.state, "cannot connect: already connected or transitioning");
            return Ok(());
        }

        tracing::info!(target = ?target, "initiating VPN connection");
        self.state = ConnectionState::Connecting;

        match self.backend.connect(target).await {
            Ok(()) => {
                // Query actual status to populate connection info.
                match self.backend.status().await {
                    Ok(status) if status.connected => {
                        let info = ConnectionInfo {
                            server: status.server.unwrap_or_default(),
                            protocol: status.protocol.unwrap_or_default(),
                            ip: status.ip.unwrap_or_default(),
                            country: status.country.unwrap_or_default(),
                            city: status.city.unwrap_or_default(),
                            connected_at: Some(Instant::now()),
                        };
                        tracing::info!(server = %info.server, "VPN connected");
                        self.state = ConnectionState::Connected(info);
                    }
                    _ => {
                        tracing::warn!("connect succeeded but status check failed");
                        self.state = ConnectionState::Disconnected;
                    }
                }
                Ok(())
            }
            Err(e) => {
                tracing::error!(error = %e, "VPN connection failed");
                self.state = ConnectionState::Disconnected;
                Err(e)
            }
        }
    }

    /// Disconnect from the VPN.
    ///
    /// Transitions: `Connected → Disconnecting → Disconnected`.
    pub async fn disconnect(&mut self) -> api::Result<()> {
        if !self.state.is_connected() {
            tracing::warn!(state = %self.state, "cannot disconnect: not connected");
            return Ok(());
        }

        tracing::info!("initiating VPN disconnection");
        self.state = ConnectionState::Disconnecting;

        match self.backend.disconnect().await {
            Ok(()) => {
                tracing::info!("VPN disconnected");
                self.state = ConnectionState::Disconnected;
                Ok(())
            }
            Err(e) => {
                tracing::error!(error = %e, "VPN disconnection failed");
                // Attempt to determine actual state.
                if let Ok(status) = self.backend.status().await {
                    if status.connected {
                        // Still connected — revert state.
                        let info = ConnectionInfo {
                            server: status.server.unwrap_or_default(),
                            protocol: status.protocol.unwrap_or_default(),
                            ip: status.ip.unwrap_or_default(),
                            country: status.country.unwrap_or_default(),
                            city: status.city.unwrap_or_default(),
                            connected_at: None,
                        };
                        self.state = ConnectionState::Connected(info);
                    } else {
                        self.state = ConnectionState::Disconnected;
                    }
                } else {
                    // Can't determine state — assume disconnected.
                    self.state = ConnectionState::Disconnected;
                }
                Err(e)
            }
        }
    }

    /// Refresh the connection state from the backend.
    ///
    /// Useful for periodic polling or initial startup.
    pub async fn refresh(&mut self) -> api::Result<()> {
        let status = self.backend.status().await?;
        if status.connected {
            // Preserve existing connected_at if already connected.
            let connected_at = self.state.info().and_then(|i| i.connected_at);
            self.state = ConnectionState::Connected(ConnectionInfo {
                server: status.server.unwrap_or_default(),
                protocol: status.protocol.unwrap_or_default(),
                ip: status.ip.unwrap_or_default(),
                country: status.country.unwrap_or_default(),
                city: status.city.unwrap_or_default(),
                connected_at: connected_at.or(Some(Instant::now())),
            });
        } else {
            self.state = ConnectionState::Disconnected;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ApiError, VpnStatus};
    use crate::servers::Server;
    use std::sync::Mutex;

    /// Mock backend for testing state transitions.
    struct MockBackend {
        connected: Mutex<bool>,
        should_fail_connect: bool,
        should_fail_disconnect: bool,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                connected: Mutex::new(false),
                should_fail_connect: false,
                should_fail_disconnect: false,
            }
        }

        fn failing_connect() -> Self {
            Self {
                connected: Mutex::new(false),
                should_fail_connect: true,
                should_fail_disconnect: false,
            }
        }

        fn failing_disconnect() -> Self {
            Self {
                connected: Mutex::new(true),
                should_fail_connect: false,
                should_fail_disconnect: true,
            }
        }
    }

    impl VpnBackend for MockBackend {
        async fn list_servers(
            &self,
            _limit: u32,
            _technology: Option<&str>,
        ) -> api::Result<Vec<Server>> {
            Ok(vec![])
        }

        async fn recommendations(
            &self,
            _limit: u32,
            _country_id: Option<u32>,
        ) -> api::Result<Vec<Server>> {
            Ok(vec![])
        }

        async fn connect(&self, _target: Option<&str>) -> api::Result<()> {
            if self.should_fail_connect {
                return Err(ApiError::Cli("mock connect failure".into()));
            }
            *self.connected.lock().unwrap() = true;
            Ok(())
        }

        async fn disconnect(&self) -> api::Result<()> {
            if self.should_fail_disconnect {
                return Err(ApiError::Cli("mock disconnect failure".into()));
            }
            *self.connected.lock().unwrap() = false;
            Ok(())
        }

        async fn status(&self) -> api::Result<VpnStatus> {
            let connected = *self.connected.lock().unwrap();
            Ok(VpnStatus {
                connected,
                server: if connected {
                    Some("us100.nordvpn.com".into())
                } else {
                    None
                },
                country: if connected {
                    Some("United States".into())
                } else {
                    None
                },
                city: if connected {
                    Some("New York".into())
                } else {
                    None
                },
                ip: if connected {
                    Some("10.0.0.1".into())
                } else {
                    None
                },
                protocol: if connected {
                    Some("NordLynx".into())
                } else {
                    None
                },
                uptime: None,
            })
        }
    }

    #[test]
    fn initial_state_is_disconnected() {
        let mgr = ConnectionManager::new(MockBackend::new());
        assert!(matches!(mgr.state(), ConnectionState::Disconnected));
    }

    #[test]
    fn state_labels() {
        assert_eq!(ConnectionState::Disconnected.label(), "Disconnected");
        assert_eq!(ConnectionState::Connecting.label(), "Connecting");
        assert_eq!(ConnectionState::Disconnecting.label(), "Disconnecting");

        let info = ConnectionInfo {
            server: "test".into(),
            protocol: "NordLynx".into(),
            ip: "1.2.3.4".into(),
            country: "US".into(),
            city: "NY".into(),
            connected_at: None,
        };
        assert_eq!(ConnectionState::Connected(info).label(), "Connected");
    }

    #[test]
    fn state_is_connected() {
        assert!(!ConnectionState::Disconnected.is_connected());
        assert!(!ConnectionState::Connecting.is_connected());
        assert!(!ConnectionState::Disconnecting.is_connected());

        let info = ConnectionInfo {
            server: "test".into(),
            protocol: "NordLynx".into(),
            ip: "1.2.3.4".into(),
            country: "US".into(),
            city: "NY".into(),
            connected_at: None,
        };
        assert!(ConnectionState::Connected(info).is_connected());
    }

    #[test]
    fn state_is_transitioning() {
        assert!(!ConnectionState::Disconnected.is_transitioning());
        assert!(ConnectionState::Connecting.is_transitioning());
        assert!(ConnectionState::Disconnecting.is_transitioning());

        let info = ConnectionInfo {
            server: "test".into(),
            protocol: "NordLynx".into(),
            ip: "1.2.3.4".into(),
            country: "US".into(),
            city: "NY".into(),
            connected_at: None,
        };
        assert!(!ConnectionState::Connected(info).is_transitioning());
    }

    #[test]
    fn state_display() {
        assert_eq!(format!("{}", ConnectionState::Disconnected), "Disconnected");
        assert_eq!(
            format!("{}", ConnectionState::Connecting),
            "Connecting..."
        );
        assert_eq!(
            format!("{}", ConnectionState::Disconnecting),
            "Disconnecting..."
        );

        let info = ConnectionInfo {
            server: "us100.nordvpn.com".into(),
            protocol: "NordLynx".into(),
            ip: "10.0.0.1".into(),
            country: "US".into(),
            city: "NY".into(),
            connected_at: None,
        };
        let display = format!("{}", ConnectionState::Connected(info));
        assert!(display.contains("us100.nordvpn.com"));
        assert!(display.contains("NordLynx"));
    }

    #[test]
    fn connection_info_uptime() {
        let info = ConnectionInfo {
            server: "test".into(),
            protocol: "NordLynx".into(),
            ip: "1.2.3.4".into(),
            country: "US".into(),
            city: "NY".into(),
            connected_at: Some(Instant::now()),
        };
        // Uptime should be very small (just created).
        let uptime = info.uptime().unwrap();
        assert!(uptime.as_secs() < 1);
    }

    #[test]
    fn connection_info_no_uptime_without_instant() {
        let info = ConnectionInfo {
            server: "test".into(),
            protocol: "NordLynx".into(),
            ip: "1.2.3.4".into(),
            country: "US".into(),
            city: "NY".into(),
            connected_at: None,
        };
        assert!(info.uptime().is_none());
    }

    #[tokio::test]
    async fn connect_transitions_to_connected() {
        let mut mgr = ConnectionManager::new(MockBackend::new());
        mgr.connect(None).await.unwrap();
        assert!(mgr.state().is_connected());

        let info = mgr.state().info().unwrap();
        assert_eq!(info.server, "us100.nordvpn.com");
        assert_eq!(info.protocol, "NordLynx");
    }

    #[tokio::test]
    async fn disconnect_transitions_to_disconnected() {
        let mut mgr = ConnectionManager::new(MockBackend::new());
        mgr.connect(None).await.unwrap();
        assert!(mgr.state().is_connected());

        mgr.disconnect().await.unwrap();
        assert!(matches!(mgr.state(), ConnectionState::Disconnected));
    }

    #[tokio::test]
    async fn connect_failure_reverts_to_disconnected() {
        let mut mgr = ConnectionManager::new(MockBackend::failing_connect());
        let result = mgr.connect(None).await;
        assert!(result.is_err());
        assert!(matches!(mgr.state(), ConnectionState::Disconnected));
    }

    #[tokio::test]
    async fn disconnect_when_not_connected_is_noop() {
        let mut mgr = ConnectionManager::new(MockBackend::new());
        mgr.disconnect().await.unwrap();
        assert!(matches!(mgr.state(), ConnectionState::Disconnected));
    }

    #[tokio::test]
    async fn connect_when_already_connected_is_noop() {
        let mut mgr = ConnectionManager::new(MockBackend::new());
        mgr.connect(None).await.unwrap();
        assert!(mgr.state().is_connected());

        // Second connect should be a no-op.
        mgr.connect(Some("de50")).await.unwrap();
        assert!(mgr.state().is_connected());
        // Should still be on the original server.
        assert_eq!(mgr.state().info().unwrap().server, "us100.nordvpn.com");
    }

    #[tokio::test]
    async fn refresh_updates_state() {
        let mut mgr = ConnectionManager::new(MockBackend::new());
        // Manually set the backend to connected.
        *mgr.backend.connected.lock().unwrap() = true;

        mgr.refresh().await.unwrap();
        assert!(mgr.state().is_connected());
        assert_eq!(
            mgr.state().info().unwrap().server,
            "us100.nordvpn.com"
        );
    }

    #[tokio::test]
    async fn refresh_detects_disconnect() {
        let mut mgr = ConnectionManager::new(MockBackend::new());
        // Connect first.
        mgr.connect(None).await.unwrap();
        assert!(mgr.state().is_connected());

        // Backend disconnects externally.
        *mgr.backend.connected.lock().unwrap() = false;

        mgr.refresh().await.unwrap();
        assert!(matches!(mgr.state(), ConnectionState::Disconnected));
    }
}
