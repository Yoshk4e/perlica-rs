use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPatchEntry {
    #[serde(rename = "skillId")]
    pub skill_id: String,
    #[serde(rename = "skillName")]
    pub skill_name: LocalizedText,
    pub level: u32,
    #[serde(rename = "scriptObjectName")]
    pub script_object_name: String,
    pub description: LocalizedText,
    #[serde(rename = "iconId")]
    pub icon_id: String,
    #[serde(rename = "iconBgType")]
    pub icon_bg_type: u32,
    #[serde(rename = "costType")]
    pub cost_type: u32,
    #[serde(rename = "costValue")]
    pub cost_value: u32,
    #[serde(rename = "coolDown")]
    pub cool_down: u32,
    #[serde(rename = "maxChargeTime")]
    pub max_charge_time: u32,
    pub blackboard: Vec<BlackboardEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardEntry {
    pub key: String,
    pub value: f64,
    #[serde(rename = "valueStr")]
    pub value_str: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizedText {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPatchBundle {
    #[serde(rename = "SkillPatchDataBundle")]
    pub entries: Vec<SkillPatchEntry>,
}

// Top-level: HashMap<skillId, SkillPatchBundle>
pub type SkillPatchTable = HashMap<String, SkillPatchBundle>;
