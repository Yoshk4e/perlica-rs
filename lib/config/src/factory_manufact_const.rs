use crate::error::{ConfigError, Result};
use crate::tables::factory_manufact_const::FManufactConst;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FManufactConstAssets {
    pub data: FManufactConst,
}

impl FManufactConstAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("FacManufactConst.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;
        let data: FManufactConst =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;
        Ok(Self { data })
    }
}
