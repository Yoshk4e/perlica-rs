use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponTable {
    #[serde(rename = "weaponBasicTable")]
    pub weapon_basic_table: HashMap<String, Weapon>,

    #[serde(rename = "weaponExpItemTable")]
    pub weapon_exp_item_table: HashMap<String, WeaponExpItem>,

    #[serde(rename = "weaponUpgradeTemplateSumTable")]
    pub weapon_upgrade_template_sum_table: HashMap<String, UpgradeTemplateSum>,

    #[serde(rename = "weaponUpgradeTemplateTable")]
    pub weapon_upgrade_template_table: HashMap<String, UpgradeTemplate>,

    #[serde(rename = "weaponBreakThroughTemplateTable")]
    pub weapon_break_through_template_table: HashMap<String, BreakthroughTemplate>,

    #[serde(rename = "weaponAttrTemplateTable")]
    pub weapon_attr_template_table: HashMap<String, AttrTemplate>,

    #[serde(rename = "GemTable")]
    pub gem_table: HashMap<String, Gem>,

    #[serde(rename = "GemTermTable")]
    pub gem_term_table: HashMap<String, GemTerm>,

    #[serde(rename = "DropGemTable")]
    pub drop_gem_table: HashMap<String, DropGemType>,

    #[serde(rename = "GemItemTable")]
    pub gem_item_table: HashMap<String, GemItem>,

    #[serde(rename = "TermEffectTable")]
    pub term_effect_table: HashMap<String, TermEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gem {
    #[serde(rename = "gemTermId")]
    pub gem_term_id: String,
    #[serde(rename = "termEffect")]
    pub term_effect: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GemTerm {
    #[serde(rename = "dataBundle")]
    pub data_bundle: Vec<GemTermEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GemTermEntry {
    pub id: String,
    #[serde(rename = "effectId")]
    pub effect_id: String,
    pub param: f64,
    pub cost: u32,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weapon {
    #[serde(rename = "weaponId")]
    pub weapon_id: String,
    #[serde(rename = "levelTemplateId")]
    pub level_template_id: String,
    #[serde(rename = "breakthroughTemplateId")]
    pub breakthrough_template_id: String,
    #[serde(rename = "mainAttrTemplateId")]
    pub main_attr_template_id: String,
    #[serde(rename = "subAttrTemplateId")]
    pub sub_attr_template_id: Vec<String>,
    #[serde(rename = "weaponSkillId")]
    pub weapon_skill_id: String,
    #[serde(rename = "weaponType")]
    pub weapon_type: u32,
    pub rarity: u32,
    #[serde(rename = "maxLv")]
    pub max_lv: u32,
    #[serde(rename = "modelPath")]
    pub model_path: String,
    #[serde(rename = "weaponDesc")]
    pub weapon_desc: LocalizedText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponExpItem {
    #[serde(rename = "expItemId")]
    pub exp_item_id: String,
    #[serde(rename = "itemExp")]
    pub item_exp: u32,
    #[serde(rename = "weaponExp")]
    pub weapon_exp: u32,
    #[serde(rename = "weaponExpConvertRatio")]
    pub weapon_exp_convert_ratio: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeTemplateSum {
    pub list: Vec<UpgradeSumItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeSumItem {
    #[serde(rename = "weaponLv")]
    pub weapon_lv: u32,
    #[serde(rename = "lvUpExpSum")]
    pub lv_up_exp_sum: u32,
    #[serde(rename = "lvUpGoldSum")]
    pub lv_up_gold_sum: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeTemplate {
    pub list: Vec<UpgradeItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeItem {
    #[serde(rename = "weaponLv")]
    pub weapon_lv: u32,
    #[serde(rename = "lvUpExp")]
    pub lv_up_exp: u32,
    #[serde(rename = "lvUpGold")]
    pub lv_up_gold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakthroughTemplate {
    pub list: Vec<BreakthroughItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakthroughItem {
    #[serde(rename = "breakthroughLv")]
    pub breakthrough_lv: u32,
    #[serde(rename = "breakthroughShowLv")]
    pub breakthrough_show_lv: u32,
    #[serde(rename = "breakthroughGold")]
    pub breakthrough_gold: u32,
    #[serde(rename = "breakItemList")]
    pub break_item_list: Vec<BreakItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakItem {
    pub id: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttrTemplate {
    pub list: Vec<AttrEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttrEntry {
    #[serde(rename = "attrType")]
    pub attr_type: u32,
    #[serde(rename = "modifierType")]
    pub modifier_type: u32,
    #[serde(rename = "initValue")]
    pub init_value: f64,
    #[serde(rename = "initLv")]
    pub init_lv: u32,
    #[serde(rename = "maxRangeLv")]
    pub max_range_lv: u32,
    #[serde(rename = "addValue")]
    pub add_value: f64,
    #[serde(rename = "breakthroughAddValue")]
    pub breakthrough_add_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropGemType {
    #[serde(rename = "dropGemTypeId")]
    pub drop_gem_type_id: String,
    #[serde(rename = "dropTerms")]
    pub drop_terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GemItem {
    #[serde(rename = "gemItemId")]
    pub gem_item_id: String,
    #[serde(rename = "dropGemTypeId")]
    pub drop_gem_type_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermEffect {
    #[serde(rename = "effectId")]
    pub effect_id: String,
    #[serde(rename = "effectType")]
    pub effect_type: u32,
    #[serde(rename = "skillModifierId")]
    pub skill_modifier_id: String,
    #[serde(rename = "attrId")]
    pub attr_id: u32,
    #[serde(rename = "calcType")]
    pub calc_type: u32,
    #[serde(rename = "attrModifier")]
    pub attr_modifier: u32,
    #[serde(rename = "limitType")]
    pub limit_type: u32,
    pub name: LocalizedText,
    pub desc: LocalizedText,
    #[serde(rename = "descStatic")]
    pub desc_static: LocalizedText,
    #[serde(rename = "superiorType")]
    pub superior_type: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizedText {
    pub text: String,
    pub id: String,
}
