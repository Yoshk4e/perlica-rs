use crate::error::{ConfigError, Result};
use crate::tables::factory_processor_const::FProcessorConst;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FProcessorConstAssets {
    pub data: FProcessorConst,
}

impl FProcessorConstAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("FacProcessorConst.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;
        let data: FProcessorConst =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;
        Ok(Self { data })
    }
}
