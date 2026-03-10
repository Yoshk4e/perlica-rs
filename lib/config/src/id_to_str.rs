use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumIdStrTable {
    #[serde(flatten)]
    pub categories: HashMap<String, NumIdStrCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumIdStrCategory {
    pub dic: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct NumIdStrAssets {
    data: NumIdStrTable,
}

impl NumIdStrAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("NumIdStingTable.json");
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let table: NumIdStrTable = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        Ok(Self { data: table })
    }

    pub fn get_str(&self, category: &str, id: u32) -> Option<&str> {
        self.data
            .categories
            .get(category)?
            .dic
            .get(&id.to_string())
            .map(|s| s.as_str())
    }

    pub fn get_item_str(&self, id: u32) -> Option<&str> {
        self.get_str("item_id", id)
    }

    pub fn get_char_str(&self, id: u32) -> Option<&str> {
        self.get_str("char_id", id)
    }
}
