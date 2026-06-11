//! Each contract bundles together:
//!   - the random-order pool (`orderIdGroup` * `orderWeight`)
//!   - the per-item value contributions (`itemValueGroup`)
//!   - the rewards (`rewardItems`, 3 entries: bloc_gold, gold, bloc_exp)
//!   - the soft min/max order value gates

use serde::Deserialize;
use std::collections::HashMap;

use crate::tables::factory_table::LocalizedText;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractEntry {
    pub contract_id: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub front_icon: String,
    #[serde(default)]
    pub contract_name: Option<LocalizedText>,
    #[serde(default)]
    pub desc: Option<LocalizedText>,
    pub trader_level: u32,
    /// 0 = medicine contract, 1 = tool contract.
    #[serde(rename = "type")]
    pub contract_type: u32,
    #[serde(default)]
    pub order_id_group: Vec<String>,
    /// Parallel array to `order_id_group`, used for weighted-random pick.
    #[serde(default)]
    pub order_weight: Vec<u32>,
    #[serde(default)]
    pub item_value_group: Vec<ItemValueEntry>,
    #[serde(default)]
    pub needed_items: Vec<String>,
    #[serde(default)]
    pub reward_items: Vec<String>,
    pub min_value: u64,
    pub max_value: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemValueEntry {
    pub item_id: String,
    pub value: u64,
}

pub type ContractTable = HashMap<String, ContractEntry>;
