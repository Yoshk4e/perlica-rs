use crate::error::{Result, ServerError};
use perlica_logic::player::WorldState;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub assets: AssetsConfig,
    pub world_state: WorldState,
    pub default_team: DefaultTeamConfig,
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

impl Config {
    pub fn load() -> Result<Self> {
        let path = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "Config.toml".to_string());

        let contents = std::fs::read_to_string(&path).map_err(|e| ServerError::ConfigRead {
            path: path.clone(),
            source: e,
        })?;

        Ok(toml::from_str(&contents)?)
    }
}
