use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterTable {
    #[serde(rename = "characterTable")]
    pub character_table: HashMap<String, Character>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    #[serde(rename = "charId")]
    pub char_id: String,
    pub name: Name,
    #[serde(rename = "engName")]
    pub eng_name: String,
    pub profession: u32,
    #[serde(rename = "weaponType")]
    pub weapon_type: u32,
    pub rarity: u32,
    #[serde(rename = "energyShardType")]
    pub energy_shard_type: u32,
    #[serde(rename = "breakData")]
    pub break_data: Vec<BreakData>,
    pub attributes: Vec<Attributes>,
    #[serde(rename = "facSkills")]
    pub fac_skills: Vec<FacSkills>,
    #[serde(rename = "defaultSkill")]
    pub default_skill: Vec<DefaultSkill>,
    #[serde(rename = "skillLevelUp")]
    pub skill_level_up: Vec<SkillLevelUp>,
    #[serde(rename = "profileVoice")]
    pub profile_voice: Vec<ProfileVoice>,
    #[serde(rename = "profileRecord")]
    pub profile_record: Vec<ProfileRecord>,
    #[serde(rename = "breakStageEffect")]
    pub break_stage_effect: HashMap<String, BreakStageEffect>,
    #[serde(rename = "talentDataBundle")]
    pub talent_data_bundle: Vec<TalentDataBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Name {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakData {
    #[serde(rename = "breakStage")]
    pub break_stage: u32,
    #[serde(rename = "maxLevel")]
    pub max_level: u32,
    #[serde(rename = "requiredItem")]
    pub required_item: Vec<RequiredItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredItem {
    pub id: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attributes {
    pub level: i32,
    #[serde(rename = "breakStage")]
    pub break_stage: u32,
    pub hp: f64,
    pub atk: u32,
    pub def: u32,
    pub pen: u32,
    #[serde(rename = "physicalResistance")]
    pub physical_resistance: u32,
    #[serde(rename = "fireResistance")]
    pub fire_resistance: u32,
    #[serde(rename = "pulseResistance")]
    pub pulse_resistance: u32,
    #[serde(rename = "crystResistance")]
    pub cryst_resistance: u32,
    pub weight: u32,
    #[serde(rename = "criticalRate")]
    pub critical_rate: f32,
    #[serde(rename = "criticalDamage")]
    pub critical_damage: f32,
    #[serde(rename = "normalAttackRange")]
    pub normal_attack_range: f32,
    #[serde(rename = "attackRate")]
    pub attack_rate: u32,
    pub hatred: u32,
    #[serde(rename = "spawnEnergyShardEfficiency")]
    pub spawn_energy_shard_efficiency: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacSkills {
    #[serde(rename = "skillIndex")]
    pub skill_index: u32,
    #[serde(rename = "skillId")]
    pub skill_id: String,
    #[serde(rename = "breakStage")]
    pub break_stage: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultSkill {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLevelUp {
    #[serde(rename = "skillID")]
    pub skill_id: String,
    #[serde(rename = "skillType")]
    pub skill_type: u32,
    pub level: u32,
    #[serde(rename = "goldCost")]
    pub gold_cost: u32,
    #[serde(rename = "itemBundle")]
    pub item_bundle: Vec<ItemBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemBundle {
    pub id: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileVoice {
    #[serde(rename = "charId")]
    pub char_id: String,
    #[serde(rename = "voiceIndex")]
    pub voice_index: u32,
    #[serde(rename = "voiceID")]
    pub voice_id: String,
    #[serde(rename = "voiceDesc")]
    pub voice_desc: VoiceDesc,
    #[serde(rename = "voiceTitle")]
    pub voice_title: VoiceTitle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceDesc {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceTitle {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRecord {
    #[serde(rename = "charId")]
    pub char_id: String,
    #[serde(rename = "recordIndex")]
    pub record_index: u32,
    #[serde(rename = "recordID")]
    pub record_id: String,
    #[serde(rename = "recordDesc")]
    pub record_desc: RecordDesc,
    #[serde(rename = "recordTitle")]
    pub record_title: RecordTitle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordDesc {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordTitle {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakStageEffect {
    #[serde(rename = "breakStage")]
    pub break_stage: u32,
    #[serde(rename = "skillEffect")]
    pub skill_effect: Vec<SkillEffect>,
    #[serde(rename = "skillUnlock")]
    pub skill_unlock: Vec<String>,
    #[serde(rename = "facSkillUnlock")]
    pub fac_skill_unlock: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEffect {
    #[serde(rename = "skillType")]
    pub skill_type: u32,
    #[serde(rename = "maxLevel")]
    pub max_level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalentDataBundle {
    #[serde(rename = "talentIndex")]
    pub talent_index: u32,
    #[serde(rename = "breakStage")]
    pub break_stage: u32,
    pub rank: u32,
    #[serde(rename = "potentialRank")]
    pub potential_rank: u32,
    #[serde(rename = "talentName")]
    pub talent_name: TalentName,
    pub description: TalentDesc,
    #[serde(rename = "talentEffects")]
    pub talent_effects: Vec<TalentEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalentName {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalentDesc {
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalentEffect {
    #[serde(rename = "talentEffectType")]
    pub talent_effect_type: u32,
    #[serde(rename = "passiveSkillId")]
    pub passive_skill_id: String,
    #[serde(rename = "passiveSkillLevel")]
    pub passive_skill_level: u32,
}
