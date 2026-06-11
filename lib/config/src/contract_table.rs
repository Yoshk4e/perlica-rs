use crate::error::{ConfigError, Result};
use crate::tables::contract_table::{ContractEntry, ContractTable};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ContractAssets {
    pub data: ContractTable,
}

impl ContractAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("ContractTable.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;
        let data: ContractTable =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;
        Ok(Self { data })
    }

    pub fn get_contract(&self, contract_id: &str) -> Option<&ContractEntry> {
        self.data.get(contract_id)
    }

    /// All contracts whose `traderLevel` matches.
    pub fn contracts_by_level(&self, trader_level: u32) -> impl Iterator<Item = &ContractEntry> {
        self.data
            .values()
            .filter(move |c| c.trader_level == trader_level)
    }

    /// All contracts whose `type` matches (0 = medicine, 1 = tool).
    pub fn contracts_by_type(&self, ty: u32) -> impl Iterator<Item = &ContractEntry> {
        self.data.values().filter(move |c| c.contract_type == ty)
    }

    pub fn all_contracts(&self) -> impl Iterator<Item = &ContractEntry> {
        self.data.values()
    }
}
