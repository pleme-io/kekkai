//! Server list and selection logic.
//!
//! Fetches server list from NordVPN API, filters by country/protocol/load,
//! and selects optimal server based on latency and capacity.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Server model
// ---------------------------------------------------------------------------

/// A NordVPN server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    /// NordVPN internal server ID.
    pub id: u32,
    /// Human-readable name (e.g. "United States #1234").
    pub name: String,
    /// DNS hostname (e.g. "us1234.nordvpn.com").
    pub hostname: String,
    /// Country name.
    pub country: String,
    /// City name.
    pub city: String,
    /// Current load percentage (0-100).
    pub load: u8,
    /// Supported technologies (e.g. "wireguard_udp", "openvpn_tcp").
    pub technologies: Vec<String>,
    /// Server station IP address.
    pub ip: String,
}

impl std::fmt::Display for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}, {}) — {}% load",
            self.hostname, self.country, self.city, self.load
        )
    }
}

// ---------------------------------------------------------------------------
// Filtering
// ---------------------------------------------------------------------------

/// Criteria for filtering the server list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerFilter {
    /// Filter by country name (case-insensitive substring match).
    pub country: Option<String>,
    /// Filter by city name (case-insensitive substring match).
    pub city: Option<String>,
    /// Filter by supported protocol/technology identifier.
    pub protocol: Option<String>,
    /// Exclude servers above this load percentage.
    pub max_load: Option<u8>,
}

/// Filter servers by the given criteria.
///
/// Returns references to servers that match ALL non-`None` filter fields.
pub fn filter_servers<'a>(servers: &'a [Server], filter: &ServerFilter) -> Vec<&'a Server> {
    servers
        .iter()
        .filter(|s| {
            if let Some(ref country) = filter.country {
                if !s.country.to_lowercase().contains(&country.to_lowercase()) {
                    return false;
                }
            }
            if let Some(ref city) = filter.city {
                if !s.city.to_lowercase().contains(&city.to_lowercase()) {
                    return false;
                }
            }
            if let Some(ref protocol) = filter.protocol {
                let proto_lower = protocol.to_lowercase();
                if !s
                    .technologies
                    .iter()
                    .any(|t| t.to_lowercase().contains(&proto_lower))
                {
                    return false;
                }
            }
            if let Some(max_load) = filter.max_load {
                if s.load > max_load {
                    return false;
                }
            }
            true
        })
        .collect()
}

/// Select the best server from the list based on the filter criteria.
///
/// Among matching servers, returns the one with the lowest load.
pub fn best_server<'a>(servers: &'a [Server], filter: &ServerFilter) -> Option<&'a Server> {
    let mut candidates = filter_servers(servers, filter);
    candidates.sort_by_key(|s| s.load);
    candidates.into_iter().next()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_servers() -> Vec<Server> {
        vec![
            Server {
                id: 1,
                name: "United States #100".into(),
                hostname: "us100.nordvpn.com".into(),
                country: "United States".into(),
                city: "New York".into(),
                load: 25,
                technologies: vec!["wireguard_udp".into(), "openvpn_tcp".into()],
                ip: "10.0.0.1".into(),
            },
            Server {
                id: 2,
                name: "United States #200".into(),
                hostname: "us200.nordvpn.com".into(),
                country: "United States".into(),
                city: "Los Angeles".into(),
                load: 60,
                technologies: vec!["wireguard_udp".into()],
                ip: "10.0.0.2".into(),
            },
            Server {
                id: 3,
                name: "Germany #50".into(),
                hostname: "de50.nordvpn.com".into(),
                country: "Germany".into(),
                city: "Frankfurt".into(),
                load: 15,
                technologies: vec!["openvpn_udp".into(), "openvpn_tcp".into()],
                ip: "10.0.0.3".into(),
            },
            Server {
                id: 4,
                name: "Japan #10".into(),
                hostname: "jp10.nordvpn.com".into(),
                country: "Japan".into(),
                city: "Tokyo".into(),
                load: 80,
                technologies: vec!["wireguard_udp".into()],
                ip: "10.0.0.4".into(),
            },
        ]
    }

    #[test]
    fn filter_by_country() {
        let servers = test_servers();
        let filter = ServerFilter {
            country: Some("United States".into()),
            ..Default::default()
        };
        let result = filter_servers(&servers, &filter);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|s| s.country == "United States"));
    }

    #[test]
    fn filter_by_country_case_insensitive() {
        let servers = test_servers();
        let filter = ServerFilter {
            country: Some("germany".into()),
            ..Default::default()
        };
        let result = filter_servers(&servers, &filter);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].country, "Germany");
    }

    #[test]
    fn filter_by_city() {
        let servers = test_servers();
        let filter = ServerFilter {
            city: Some("Tokyo".into()),
            ..Default::default()
        };
        let result = filter_servers(&servers, &filter);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].hostname, "jp10.nordvpn.com");
    }

    #[test]
    fn filter_by_protocol() {
        let servers = test_servers();
        let filter = ServerFilter {
            protocol: Some("wireguard".into()),
            ..Default::default()
        };
        let result = filter_servers(&servers, &filter);
        assert_eq!(result.len(), 3);
        // Germany only has openvpn, should be excluded
        assert!(result.iter().all(|s| s.country != "Germany"));
    }

    #[test]
    fn filter_by_max_load() {
        let servers = test_servers();
        let filter = ServerFilter {
            max_load: Some(30),
            ..Default::default()
        };
        let result = filter_servers(&servers, &filter);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|s| s.load <= 30));
    }

    #[test]
    fn filter_combined() {
        let servers = test_servers();
        let filter = ServerFilter {
            country: Some("United States".into()),
            protocol: Some("wireguard".into()),
            max_load: Some(50),
            ..Default::default()
        };
        let result = filter_servers(&servers, &filter);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].hostname, "us100.nordvpn.com");
    }

    #[test]
    fn filter_no_match() {
        let servers = test_servers();
        let filter = ServerFilter {
            country: Some("Antarctica".into()),
            ..Default::default()
        };
        let result = filter_servers(&servers, &filter);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_empty_returns_all() {
        let servers = test_servers();
        let filter = ServerFilter::default();
        let result = filter_servers(&servers, &filter);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn best_server_picks_lowest_load() {
        let servers = test_servers();
        let filter = ServerFilter::default();
        let best = best_server(&servers, &filter).unwrap();
        assert_eq!(best.hostname, "de50.nordvpn.com");
        assert_eq!(best.load, 15);
    }

    #[test]
    fn best_server_with_country_filter() {
        let servers = test_servers();
        let filter = ServerFilter {
            country: Some("United States".into()),
            ..Default::default()
        };
        let best = best_server(&servers, &filter).unwrap();
        assert_eq!(best.hostname, "us100.nordvpn.com");
        assert_eq!(best.load, 25);
    }

    #[test]
    fn best_server_no_match() {
        let servers = test_servers();
        let filter = ServerFilter {
            country: Some("Antarctica".into()),
            ..Default::default()
        };
        assert!(best_server(&servers, &filter).is_none());
    }

    #[test]
    fn server_display() {
        let server = &test_servers()[0];
        let display = format!("{server}");
        assert!(display.contains("us100.nordvpn.com"));
        assert!(display.contains("United States"));
        assert!(display.contains("25%"));
    }
}
