use anyhow::{Context, Result};
use config::BeyondAssets;
use perlica_proto::{
    AttrInfo, BattleInfo, CharInfo, CharTeamInfo, CharTeamMemberInfo, ScCharSyncStatus, ScSyncAttr,
    ScSyncCharBagInfo, SkillInfo, SkillLevelInfo,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

use crate::enums::AttributeType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(transparent)]
pub struct CharIndex(u64);

impl CharIndex {
    pub fn object_id(self) -> u64 {
        self.0 + 1
    }
    pub fn from_object_id(id: u64) -> Self {
        Self(id - 1)
    }
    pub fn from_usize(idx: usize) -> Self {
        Self(idx as u64)
    }
    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct WeaponIndex(u64);

impl WeaponIndex {
    pub fn inst_id(self) -> u64 {
        self.0
    }
    pub fn from_raw(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TeamSlot {
    #[default]
    Empty,
    Occupied(CharIndex),
}

impl TeamSlot {
    pub fn char_index(&self) -> Option<CharIndex> {
        match self {
            TeamSlot::Occupied(idx) => Some(*idx),
            TeamSlot::Empty => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Team {
    pub name: String,
    pub char_team: [TeamSlot; 4],
    pub leader_index: CharIndex,
}

impl Team {
    pub const SLOTS_COUNT: usize = 4;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Char {
    pub template_id: String,
    pub level: i32,
    pub exp: i32,
    pub break_stage: u32,
    pub is_dead: bool,
    pub hp: f64,
    pub ultimate_sp: f32,
    pub weapon_id: WeaponIndex,
    pub own_time: i64,
    pub skill_levels: HashMap<String, u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    pub curr_team_index: u32,
}

#[derive(Debug, Clone)]
pub struct SkillLevelState {
    pub skill_id: String,
    pub skill_level: i32,
    pub skill_max_level: i32,
}

#[derive(Debug, Clone)]
pub struct CharSyncState {
    pub objid: u64,
    pub template_id: String,
    pub level: i32,
    pub exp: i32,
    pub break_stage: u32,
    pub hp: f64,
    pub ultimate_sp: f32,
    pub weapon_id: u64,
    pub own_time: i64,
    pub is_dead: bool,
    pub normal_skill: String,
    pub skill_levels: Vec<SkillLevelState>,
}

#[derive(Debug, Clone)]
pub struct TeamSyncState {
    pub name: String,
    pub char_ids: Vec<u64>,
    pub leader_id: u64,
    pub member_skills: HashMap<u64, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharBag {
    pub teams: Vec<Team>,
    pub chars: Vec<Char>,
    pub meta: Meta,
}

impl CharBag {
    /// Creates a fully initialized bag for a new player, populating all chars from assets.
    pub fn new(assets: &BeyondAssets) -> Result<Self> {
        let mut bag = Self::default();
        const DEFAULT_TEAM: [&str; 4] = [
            "chr_0003_endmin",
            "chr_0004_pelica",
            "chr_0005_chen",
            "chr_0006_wolfgd",
        ];

        let mut index_map: HashMap<String, CharIndex> = HashMap::new();

        info!("Starting charbag population with all characters");

        for (template_id, char_data) in assets.characters.iter() {
            if assets.char_skills.get_char_skills(template_id).is_empty() {
                debug!("Skipping placeholder char: {}", template_id);
                continue;
            }

            let attrs = match assets.characters.get_stats(template_id, 1, 0) {
                Some(a) => a,
                None => {
                    debug!("No level 1 stats for char: {}", template_id);
                    continue;
                }
            };

            let skill_levels: HashMap<String, u32> = assets
                .char_skills
                .get_char_skills(template_id)
                .into_iter()
                .filter_map(|b| b.entries.first())
                .map(|e| (e.skill_id.clone(), 1u32))
                .collect();

            let weapon = assets
                .weapons
                .get_best_for_char(char_data.weapon_type)
                .or_else(|| {
                    assets
                        .weapons
                        .get_by_type(char_data.weapon_type)
                        .first()
                        .copied()
                })
                .unwrap_or_else(|| assets.weapons.get("wpn_0002").unwrap());

            let numeric_id = assets
                .str_id_num
                .get_weapon_id(&weapon.weapon_id)
                .unwrap_or(char_data.weapon_type as u32);

            debug!(
                "Creating char: template_id={}, weapon_type={}, assigned_weapon_id={} (numeric: {}), rarity={}",
                template_id, char_data.weapon_type, weapon.weapon_id, numeric_id, weapon.rarity
            );

            let char = Char {
                template_id: template_id.clone(),
                level: attrs.level,
                exp: 0,
                break_stage: attrs.break_stage,
                is_dead: false,
                weapon_id: WeaponIndex::from_raw(numeric_id as u64),
                own_time: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0),
                hp: attrs.hp,
                ultimate_sp: 0.0,
                skill_levels,
            };

            let idx = bag.add_char(char);
            index_map.insert(template_id.clone(), idx);
        }

        debug!("Populated {} characters in charbag", index_map.len());

        let mut team = Team::default();
        team.name = "Team 1".to_string();
        let mut slot = 0;
        let mut leader = None;
        for template_id in DEFAULT_TEAM {
            if let Some(&idx) = index_map.get(template_id) {
                if slot < Team::SLOTS_COUNT {
                    team.char_team[slot] = TeamSlot::Occupied(idx);
                    if leader.is_none() {
                        leader = Some(idx);
                    }
                    slot += 1;
                }
            }
        }
        team.leader_index = leader.unwrap_or_default();
        bag.teams.push(team.clone());
        bag.meta.curr_team_index = 0;

        debug!("Default team created with leader: {:?}", team.leader_index);

        Ok(bag)
    }

    pub fn add_char(&mut self, char: Char) -> CharIndex {
        let idx = CharIndex::from_usize(self.chars.len());
        self.chars.push(char);
        idx
    }

    pub fn get_char(&self, idx: CharIndex) -> Option<&Char> {
        self.chars.get(idx.as_usize())
    }

    pub fn char_index_by_id(&self, template_id: &str) -> Option<CharIndex> {
        self.chars
            .iter()
            .position(|c| c.template_id == template_id)
            .map(CharIndex::from_usize)
    }

    pub fn get_char_by_objid_mut(&mut self, objid: u64) -> Option<&mut Char> {
        self.chars
            .get_mut(CharIndex::from_object_id(objid).as_usize())
    }

    pub fn update_battle_info(&mut self, objid: u64, hp: f64, sp: f32) {
        if let Some(char) = self.get_char_by_objid_mut(objid) {
            char.hp = hp;
            char.ultimate_sp = sp;
        }
    }

    fn team_sync_states(&self, assets: &BeyondAssets) -> Vec<TeamSyncState> {
        self.teams
            .iter()
            .map(|team| {
                let char_ids = team
                    .char_team
                    .iter()
                    .filter_map(|slot| slot.char_index())
                    .map(|idx| idx.object_id())
                    .collect();
                let member_skills = team
                    .char_team
                    .iter()
                    .filter_map(|slot| slot.char_index())
                    .map(|idx| {
                        let char_data = &self.chars[idx.as_usize()];
                        let skill = Self::get_normal_skill(&char_data.template_id, assets);
                        (idx.object_id(), skill)
                    })
                    .collect();
                TeamSyncState {
                    name: team.name.clone(),
                    char_ids,
                    leader_id: team.leader_index.object_id(),
                    member_skills,
                }
            })
            .collect()
    }

    fn char_sync_states(&self, assets: &BeyondAssets) -> Result<Vec<CharSyncState>> {
        self.chars
            .iter()
            .enumerate()
            .map(|(i, char)| {
                let objid = CharIndex::from_usize(i).object_id();
                let template = assets
                    .characters
                    .get(&char.template_id)
                    .with_context(|| format!("Unknown character template: {}", char.template_id))?;
                let bundles = assets.char_skills.get_char_skills(&template.char_id);
                let normal_skill = Self::get_normal_skill(&char.template_id, assets);
                let skill_levels = bundles
                    .iter()
                    .filter_map(|bundle| {
                        let first_id = &bundle.entries.first()?.skill_id;
                        let current_level = char.skill_levels.get(first_id).copied().unwrap_or(1);
                        let entry = bundle.entries.iter().find(|e| e.level == current_level)?;
                        let max = bundle.entries.iter().map(|e| e.level).max().unwrap_or(1);
                        Some(SkillLevelState {
                            skill_id: entry.skill_id.clone(),
                            skill_level: entry.level as i32,
                            skill_max_level: max as i32,
                        })
                    })
                    .collect();
                Ok(CharSyncState {
                    objid,
                    template_id: char.template_id.clone(),
                    level: char.level,
                    exp: char.exp,
                    break_stage: char.break_stage,
                    hp: char.hp,
                    ultimate_sp: char.ultimate_sp,
                    weapon_id: char.weapon_id.inst_id(),
                    own_time: char.own_time,
                    is_dead: char.is_dead,
                    normal_skill,
                    skill_levels,
                })
            })
            .collect()
    }

    fn get_normal_skill(template_id: &str, assets: &BeyondAssets) -> String {
        assets
            .char_skills
            .get_char_skills(template_id)
            .into_iter()
            .find_map(|b| {
                b.entries
                    .first()
                    .filter(|e| e.skill_id.contains("normal_skill"))
                    .map(|e| e.skill_id.clone())
            })
            .unwrap_or_default()
    }

    pub fn char_bag_info(&self, assets: &BeyondAssets) -> Result<ScSyncCharBagInfo> {
        let team_states = self.team_sync_states(assets);
        let char_states = self.char_sync_states(assets)?;

        let team_info = team_states
            .into_iter()
            .map(|t| CharTeamInfo {
                team_name: t.name,
                char_team: t.char_ids,
                leaderid: t.leader_id,
                member_info: t
                    .member_skills
                    .into_iter()
                    .map(|(id, skill)| {
                        (
                            id,
                            CharTeamMemberInfo {
                                normal_skillid: skill,
                            },
                        )
                    })
                    .collect(),
            })
            .collect();

        let char_info = char_states
            .into_iter()
            .map(|c| CharInfo {
                objid: c.objid,
                templateid: c.template_id,
                level: c.level,
                exp: c.exp,
                finish_break_stage: c.break_stage as i32,
                equip_col: Default::default(),
                equip_suit: Default::default(),
                normal_skill: c.normal_skill.clone(),
                is_dead: c.is_dead,
                weapon_id: c.weapon_id,
                own_time: c.own_time,
                battle_info: Some(BattleInfo {
                    hp: c.hp,
                    ultimatesp: c.ultimate_sp,
                }),
                skill_info: Some(SkillInfo {
                    normal_skill: c.normal_skill,
                    level_info: c
                        .skill_levels
                        .into_iter()
                        .map(|s| SkillLevelInfo {
                            skill_id: s.skill_id,
                            skill_level: s.skill_level,
                            skill_max_level: s.skill_max_level,
                        })
                        .collect(),
                }),
            })
            .collect();

        Ok(ScSyncCharBagInfo {
            char_info,
            team_info,
            curr_team_index: self.meta.curr_team_index as i32,
            max_char_team_member_count: Team::SLOTS_COUNT as u32,
        })
    }

    pub fn char_attrs(&self, assets: &BeyondAssets) -> Vec<ScSyncAttr> {
        self.chars
            .iter()
            .enumerate()
            .map(|(i, char)| {
                let objid = CharIndex::from_usize(i).object_id();
                let attr_list = assets
                    .characters
                    .get_stats(&char.template_id, char.level, char.break_stage)
                    .map(attrs_from_stats)
                    .unwrap_or_default();
                ScSyncAttr {
                    obj_id: objid,
                    attr_list,
                }
            })
            .collect()
    }

    pub fn char_status(&self) -> Vec<ScCharSyncStatus> {
        self.chars
            .iter()
            .enumerate()
            .map(|(i, char)| ScCharSyncStatus {
                objid: CharIndex::from_usize(i).object_id(),
                is_dead: char.is_dead,
                battle_info: Some(BattleInfo {
                    hp: char.hp,
                    ultimatesp: char.ultimate_sp,
                }),
            })
            .collect()
    }
}

fn attrs_from_stats(a: &config::tables::character::Attributes) -> Vec<AttrInfo> {
    let attr = |attr_type: AttributeType, value: f64| AttrInfo {
        attr_type: attr_type as i32,
        basic_value: value,
        value,
    };

    vec![
        attr(AttributeType::Hp, a.hp),
        attr(AttributeType::Atk, a.atk as f64),
        attr(AttributeType::Def, a.def as f64),
        attr(
            AttributeType::PhysicalResistance,
            a.physical_resistance as f64,
        ),
        attr(AttributeType::FireResistance, a.fire_resistance as f64),
        attr(AttributeType::PulseResistance, a.pulse_resistance as f64),
        attr(AttributeType::CrystResistance, a.cryst_resistance as f64),
        attr(AttributeType::Weight, a.weight as f64),
        attr(AttributeType::CriticalRate, a.critical_rate as f64),
        attr(AttributeType::CriticalDamage, a.critical_damage as f64),
        attr(AttributeType::Hatred, a.hatred as f64),
        attr(
            AttributeType::NormalAttackRange,
            a.normal_attack_range as f64,
        ),
        attr(AttributeType::AttackRate, a.attack_rate as f64),
        attr(AttributeType::Pen, a.pen as f64),
        attr(
            AttributeType::SpawnEnergyShardEfficiency,
            a.spawn_energy_shard_efficiency as f64,
        ),
    ]
}
