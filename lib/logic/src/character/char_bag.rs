use anyhow::{Context, Result};
use config::BeyondAssets;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl Default for CharIndex {
    fn default() -> Self {
        CharIndex(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WeaponIndex(u64);

impl WeaponIndex {
    pub fn inst_id(self) -> u64 {
        self.0
    }
}

impl Default for WeaponIndex {
    fn default() -> Self {
        WeaponIndex(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamSlot {
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

impl Default for TeamSlot {
    fn default() -> Self {
        TeamSlot::Empty
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub name: String,
    pub char_team: [TeamSlot; 4],
    pub leader_index: CharIndex,
}

impl Default for Team {
    fn default() -> Self {
        Self {
            name: String::new(),
            char_team: [TeamSlot::default(); 4],
            leader_index: CharIndex::default(),
        }
    }
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_starter(assets: &BeyondAssets, _uid: &str) -> Result<Self> {
        let mut bag = Self::default();

        let starter_id = "chr_0004_pelica";
        assets
            .characters
            .get(starter_id)
            .context("Starter character template not found")?;

        let attrs = assets
            .characters
            .get_stats(starter_id, 1, 0)
            .context("Starter attributes not found for level 1, break 0")?;

        let skill_levels = assets
            .char_skills
            .get_char_skills(starter_id)
            .into_iter()
            .filter_map(|b| b.entries.first())
            .map(|e| (e.skill_id.clone(), 1u32))
            .collect();

        let starter = Char {
            template_id: starter_id.to_string(),
            level: attrs.level,
            exp: 200,
            break_stage: attrs.break_stage,
            is_dead: false,
            weapon_id: WeaponIndex::default(),
            own_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
            hp: attrs.hp,
            ultimate_sp: 0.0,
            skill_levels,
        };

        let char_idx = bag.add_char(starter);
        let mut team = Team::default();
        team.name = "Team 1".to_string();
        team.char_team[0] = TeamSlot::Occupied(char_idx);
        team.leader_index = char_idx;
        bag.teams.push(team);
        bag.meta.curr_team_index = 0;

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

    pub fn prepare_team_sync_states(&self, assets: &BeyondAssets) -> Vec<TeamSyncState> {
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

    pub fn prepare_char_sync_states(&self, assets: &BeyondAssets) -> Result<Vec<CharSyncState>> {
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
}
