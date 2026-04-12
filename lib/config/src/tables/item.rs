use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct ItemFile {
    #[serde(rename = "itemTable")]
    pub item_table: HashMap<String, RawItemEntry>,
    #[serde(rename = "itemListByTypeTable", default)]
    pub item_list_by_type: HashMap<String, ItemListWrapper>,
    #[serde(rename = "expItemDataMap", default)]
    pub exp_item_data_map: HashMap<String, RawExpItemData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemListWrapper {
    pub list: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I18nText {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawItemEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: u32,
    #[serde(rename = "showingType", default)]
    pub showing_type: u32,
    pub name: I18nText,
    pub rarity: u32,
    #[serde(rename = "sortId1", default)]
    pub sort_id1: i32,
    #[serde(rename = "sortId2", default)]
    pub sort_id2: i32,
    #[serde(rename = "iconId", default)]
    pub icon_id: String,
    pub desc: I18nText,
    /// -1 = unlimited.
    #[serde(rename = "maxBackpackStackCount", default)]
    pub max_backpack_stack_count: i32,
    /// -1 = unlimited; 1 = instanced item (weapons, gems, equips).
    #[serde(rename = "maxStackCount", default)]
    pub max_stack_count: i32,
    #[serde(rename = "backpackCanDiscard", default)]
    pub backpack_can_discard: bool,
    #[serde(default)]
    pub price: u64,
    #[serde(rename = "modelKey", default)]
    pub model_key: String,
    /// 0 = Invalid, 1 = Weapon, 2 = WeaponGem, 3 = Equip, 4 = SpecialItem, 5 = MissionItem, 6 = Factory.
    #[serde(rename = "valuableTabType", default)]
    pub valuable_tab_type: u32,
    #[serde(rename = "obtainWayIds", default)]
    pub obtain_way_ids: Vec<String>,
    #[serde(rename = "noObtainWayHint")]
    pub no_obtain_way_hint: Option<I18nText>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawExpItemData {
    #[serde(rename = "expGain")]
    pub exp_gain: i64,
    #[serde(rename = "expType")]
    pub exp_type: u32,
}
