//! These are the *pre-placed* broken buildings the player has to
//! pay to activate.  Per the implementation plan there are exactly 7
//! entries currently:
//!   - 5 * `power_pole_2` in `map01_lv005`
//!   - 1 * `sp_hub_1` in `map01_lv001`
//!   - 1 * `sp_proc_sta_1` in `map01_lv001`

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairEntry {
    pub id: String,
    pub level_id: String,
    pub building_id: String,
    #[serde(default)]
    pub cost_items: Vec<RepairCostItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepairCostItem {
    pub id: String,
    pub count: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RepairTable {
    #[serde(default)]
    pub repair_building_table: HashMap<String, RepairEntry>,
}
