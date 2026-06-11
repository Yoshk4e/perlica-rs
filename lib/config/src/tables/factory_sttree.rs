use serde::Deserialize;
use std::collections::HashMap;

use crate::tables::factory_table::{ItemCount, LocalizedText};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacSttParameter {
    #[serde(default)]
    pub value_type: i32,
    #[serde(default)]
    pub real_type: i32,
    #[serde(default)]
    pub value_string_list: Option<Vec<String>>,
    #[serde(default)]
    pub value_int_list: Option<Vec<i64>>,
    #[serde(default)]
    pub value_float_list: Option<Vec<f64>>,
    #[serde(default)]
    pub value_bool_list: Option<Vec<bool>>,
    #[serde(default)]
    pub value_vector3: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacSttConditionEntry {
    pub condition_id: String,
    #[serde(default)]
    pub desc: Option<LocalizedText>,
    /// 504 = "have N of building X placed", 511 = "produce N total items", etc.
    pub condition_type: i32,
    #[serde(default)]
    pub parameters: Vec<FacSttParameter>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacSttActionEntry {
    pub action_id: String,
    /// 501 = "give blueprint items", etc.
    pub action_type: i32,
    #[serde(default)]
    pub parameters: Vec<FacSttParameter>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacSttGroupEntry {
    pub group_id: String,
    #[serde(default)]
    pub group_name: Option<LocalizedText>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacSttLayerEntry {
    pub layer_id: String,
    #[serde(default)]
    pub name: Option<LocalizedText>,
    #[serde(default)]
    pub pre_layer: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacSttNodeEntry {
    pub tech_id: String,
    #[serde(default)]
    pub name: Option<LocalizedText>,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub desc: Option<LocalizedText>,
    #[serde(default)]
    pub is_big: bool,
    #[serde(default)]
    pub sort_id: u32,
    #[serde(default)]
    pub unlock_reward: Vec<UnlockRewardItem>,
    #[serde(default)]
    pub conditions: Vec<FacSttConditionEntry>,
    #[serde(default)]
    pub unlock_desc: Option<LocalizedText>,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub pre_node: Vec<String>,
    #[serde(default)]
    pub ui_pos: Vec<i32>,
    #[serde(default)]
    pub cost_items: Vec<ItemCount>,
    #[serde(default)]
    pub action: Option<FacSttActionEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacSttSpecialNodeEntry {
    pub tech_id: String,
    #[serde(default)]
    pub name: Option<LocalizedText>,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub desc: Option<LocalizedText>,
    #[serde(default)]
    pub unlock_reward: Vec<UnlockRewardItem>,
    #[serde(default)]
    pub conditions: Vec<FacSttConditionEntry>,
    #[serde(default)]
    pub index: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockRewardItem {
    pub item_id: String,
    #[serde(default)]
    pub count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactorySttTree {
    #[serde(default, rename = "facSTTGroupTable")]
    pub group_table: HashMap<String, FacSttGroupEntry>,
    #[serde(default, rename = "facSTTLayerTable")]
    pub layer_table: HashMap<String, FacSttLayerEntry>,
    #[serde(default, rename = "facSTTNodeTable")]
    pub node_table: HashMap<String, FacSttNodeEntry>,
    #[serde(default, rename = "facSTTSpNodeTable")]
    pub sp_node_table: HashMap<String, FacSttSpecialNodeEntry>,
    #[serde(default, rename = "facSTTConditionTable")]
    pub condition_table: HashMap<String, FacSttConditionEntry>,
}
