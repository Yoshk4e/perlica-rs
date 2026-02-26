use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub assets: AssetsConfig,
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

impl Config {
    pub fn load() -> Result<Self> {
        let path = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "Config.toml".to_string());

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config: {path}"))?;

        toml::from_str(&contents).context("failed to parse Config.toml")
    }
}
