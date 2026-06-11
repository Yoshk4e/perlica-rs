use crate::error::{ConfigError, Result};
use crate::tables::repair_table::{RepairEntry, RepairTable};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RepairAssets {
    pub data: RepairTable,
}

impl RepairAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("Repair.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;
        let data: RepairTable =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;
        Ok(Self { data })
    }

    pub fn get_repair(&self, repair_id: &str) -> Option<&RepairEntry> {
        self.data.repair_building_table.get(repair_id)
    }

    pub fn entries_for_level<'a>(
        &'a self,
        level_id: &'a str,
    ) -> impl Iterator<Item = &'a RepairEntry> {
        self.data
            .repair_building_table
            .values()
            .filter(move |e| e.level_id == level_id)
    }

    pub fn all_entries(&self) -> impl Iterator<Item = &RepairEntry> {
        self.data.repair_building_table.values()
    }
}
