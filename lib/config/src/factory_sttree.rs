use crate::error::{ConfigError, Result};
use crate::tables::factory_sttree::{
    FacSttConditionEntry, FacSttGroupEntry, FacSttLayerEntry, FacSttNodeEntry,
    FacSttSpecialNodeEntry, FactorySttTree,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FSttreeAssets {
    pub data: FactorySttTree,
}

impl FSttreeAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("FactorySTTree.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;
        let data: FactorySttTree =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;
        Ok(Self { data })
    }

    pub fn get_group(&self, group_id: &str) -> Option<&FacSttGroupEntry> {
        self.data.group_table.get(group_id)
    }

    pub fn get_layer(&self, layer_id: &str) -> Option<&FacSttLayerEntry> {
        self.data.layer_table.get(layer_id)
    }

    pub fn get_node(&self, tech_id: &str) -> Option<&FacSttNodeEntry> {
        self.data.node_table.get(tech_id)
    }

    pub fn get_sp_node(&self, tech_id: &str) -> Option<&FacSttSpecialNodeEntry> {
        self.data.sp_node_table.get(tech_id)
    }

    pub fn get_condition(&self, condition_id: &str) -> Option<&FacSttConditionEntry> {
        self.data.condition_table.get(condition_id)
    }

    pub fn all_node_ids(&self) -> impl Iterator<Item = &str> {
        self.data.node_table.keys().map(String::as_str)
    }
}
