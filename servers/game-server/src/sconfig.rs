use crate::error::{Result, ServerError};
use perlica_logic::player::WorldState;
use serde::Deserialize;
use std::path::PathBuf;
use tracing::error;
static DEFAULT_CONFIG: &str = include_str!("../config.default.toml");

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub assets: AssetsConfig,
    pub world_state: WorldState,
    pub default_team: DefaultTeamConfig,
    #[serde(default)]
    pub muip: MuipGmConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl ServerConfig {
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Debug, Deserialize)]
pub struct AssetsConfig {
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct DefaultTeamConfig {
    pub team: [String; 4],
}

#[derive(Debug, Clone, Deserialize)]
pub struct MuipGmConfig {
    #[serde(default = "MuipGmConfig::default_host")]
    pub host: String,
    #[serde(default = "MuipGmConfig::default_port")]
    pub port: u16,
    #[serde(default = "MuipGmConfig::default_enabled")]
    pub enabled: bool,
}

impl Default for MuipGmConfig {
    fn default() -> Self {
        Self {
            host: Self::default_host(),
            port: Self::default_port(),
            enabled: Self::default_enabled(),
        }
    }
}

impl MuipGmConfig {
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    fn default_host() -> String {
        "127.0.0.1".to_owned()
    }
    fn default_port() -> u16 {
        2338
    }
    fn default_enabled() -> bool {
        true
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "Config.toml".to_owned());

        if !std::path::Path::new(&path).exists() {
            std::fs::write(&path, DEFAULT_CONFIG).map_err(|e| ServerError::ConfigRead {
                path: path.clone(),
                source: e,
            })?;
            error!(
                "No config found. A default one has been written to `{path}`. Edit it and restart."
            );
            std::process::exit(0);
        }

        let contents = std::fs::read_to_string(&path).map_err(|e| ServerError::ConfigRead {
            path: path.clone(),
            source: e,
        })?;

        Ok(toml::from_str(&contents)?)
    }
}
