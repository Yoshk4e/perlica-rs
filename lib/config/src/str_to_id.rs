use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrIdNumTable {
    #[serde(flatten)]
    pub categories: HashMap<String, StrIdNumCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrIdNumCategory {
    pub dic: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
pub struct StrIdNumAssets {
    data: StrIdNumTable,
}

impl StrIdNumAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("StrIdNumTable.json");
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let table: StrIdNumTable = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        Ok(Self { data: table })
    }

    // Get numeric ID for any string key in any category
    pub fn get_id(&self, category: &str, key: &str) -> Option<u32> {
        self.data.categories.get(category)?.dic.get(key).copied()
    }

    // Convenience: weapon numeric ID (comes from "item_id" category)
    pub fn get_weapon_id(&self, weapon_id: &str) -> Option<u32> {
        self.get_id("item_id", weapon_id)
    }

    // Convenience: character numeric ID
    pub fn get_char_id(&self, char_id: &str) -> Option<u32> {
        self.get_id("char_id", char_id)
    }

    pub fn get_scene_id(&self, scene_id: &str) -> Option<u64> {
        self.get_id("scene_name_id", scene_id).map(|v| v as u64)
    }
}
