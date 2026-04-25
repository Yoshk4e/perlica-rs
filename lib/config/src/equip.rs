use crate::error::{ConfigError, Result};
use crate::tables::equip::{EquipBasicEntry, EquipConfig, EquipEntry};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct EquipmentAssets {
    data: EquipConfig,
}

impl EquipmentAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("Equip.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;

        let table: EquipConfig =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;

        Ok(Self { data: table })
    }

    #[inline]
    pub fn get_equip(&self, id: &str) -> Option<&EquipEntry> {
        self.data.equip_table.get(id)
    }

    #[inline]
    pub fn get_basic(&self, id: &str) -> Option<&EquipBasicEntry> {
        self.data
            .equip_basic_table
            .get(id)
            .or_else(|| self.data.equip_table.get(id).map(|e| &e.equip_basic_entry))
    }

    //Might need it later idk, but now its useless
    /*fn get_attrs(&self, id: &str) -> Option<&[AttrModifier]> {
        self.get_equip(id).map(|e| e.attr_modifiers.as_slice())
    }*/
}
