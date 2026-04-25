use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipConfig {
    pub equip_basic_table: HashMap<String, EquipBasicEntry>,
    pub equip_table: HashMap<String, EquipEntry>,
    pub equip_suit_table: HashMap<String, EquipSuitEntry>,
    pub equip_tier_table: HashMap<String, EquipTierEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizedText {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipBasicEntry {
    pub item_id: String,
    pub part_type: i32,
    pub name: LocalizedText,
    pub attach_slot_num: i32,
    #[serde(rename = "suitID")]
    pub suit_id: String,
    #[serde(rename = "tierId")]
    pub tier_id: String,
    #[serde(default)]
    pub enhance_from: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipEntry {
    #[serde(flatten)]
    pub equip_basic_entry: EquipBasicEntry,
    pub attr_modifiers: Vec<AttrModifier>,
    pub enhance_infos: Vec<EnhanceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttrModifier {
    pub attr_type: i32,
    pub attr_value: f64,
    pub modifier_type: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnhanceInfo {
    pub attr_type: i32,
    pub ratios: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipSuitEntry {
    pub list: Vec<SuitList>,
    pub equip_list: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuitList {
    #[serde(rename = "suitID")]
    pub suit_id: String,
    pub equip_cnt: i32,
    pub desc: LocalizedText,
    pub suit_name: LocalizedText,
    #[serde(rename = "skillID")]
    pub skill_id: String,
    pub skill_lv: i32,
    pub suit_icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipTierEntry {
    #[serde(rename = "tierID")]
    pub tier_id: String,
    pub tier_desc: String,
    pub min_wear_lv: i32,
}
