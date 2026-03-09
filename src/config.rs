use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KekkaiConfig {
    #[serde(default)]
    pub connection: ConnectionConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Path to `nordvpn` CLI binary
    #[serde(default = "default_nordvpn_path")]
    pub nordvpn_path: String,
    /// Preferred protocol (NordLynx/OpenVPN)
    #[serde(default = "default_protocol")]
    pub protocol: String,
    /// Preferred country code for quick-connect
    pub preferred_country: Option<String>,
    /// Auto-connect on launch
    #[serde(default)]
    pub auto_connect: bool,
    /// Kill switch
    #[serde(default = "default_killswitch")]
    pub killswitch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    #[serde(default = "default_bg")]
    pub background: String,
    #[serde(default = "default_fg")]
    pub foreground: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_connected_color")]
    pub connected_color: String,
}

impl Default for KekkaiConfig {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig::default(),
            appearance: AppearanceConfig::default(),
        }
    }
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            nordvpn_path: default_nordvpn_path(),
            protocol: default_protocol(),
            preferred_country: None,
            auto_connect: false,
            killswitch: default_killswitch(),
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            background: default_bg(),
            foreground: default_fg(),
            accent: default_accent(),
            connected_color: default_connected_color(),
        }
    }
}

fn default_nordvpn_path() -> String { "nordvpn".into() }
fn default_protocol() -> String { "NordLynx".into() }
fn default_killswitch() -> bool { true }
fn default_bg() -> String { "#2e3440".into() }
fn default_fg() -> String { "#eceff4".into() }
fn default_accent() -> String { "#88c0d0".into() }
fn default_connected_color() -> String { "#a3be8c".into() }

pub fn load(override_path: &Option<PathBuf>) -> anyhow::Result<KekkaiConfig> {
    let path = match override_path {
        Some(p) => p.clone(),
        None => match shikumi::ConfigDiscovery::new("kekkai")
            .env_override("KEKKAI_CONFIG")
            .discover()
        {
            Ok(p) => p,
            Err(_) => {
                tracing::info!("no config file found, using defaults");
                return Ok(KekkaiConfig::default());
            }
        },
    };

    let store = shikumi::ConfigStore::<KekkaiConfig>::load(&path, "KEKKAI_")?;
    Ok(KekkaiConfig::clone(&store.get()))
}
