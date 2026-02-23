use anyhow::{Context, Result};
use config::BeyondAssets;
use perlica_proto::ItemInst;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// Type-safe index wrapper
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

// Weapon index (adjust based on your actual WeaponIndex type)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WeaponIndex(u64);

impl WeaponIndex {
    pub fn inst_id(self) -> u64 {
        self.0
    }
}

impl Default for WeaponIndex {
    fn default() -> Self {
        WeaponIndex(0) // 0 = no weapon / invalid weapon
    }
}

// Team slot
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

// Team configuration
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
            char_team: [TeamSlot::default(); 4], // 4 empty slots
            leader_index: CharIndex(0),          // or CharIndex::default() if you implement it
        }
    }
}

impl Team {
    pub const SLOTS_COUNT: usize = 4;
}

// Character instance data
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

// Metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    pub curr_team_index: u32,
}

// Main CharBag - player's character collection
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

    pub fn new_with_starter(assets: &BeyondAssets, uid: &str) -> Result<Self> {
        let mut bag = Self::default();

        // Add a guaranteed starter character (e.g., MC or first free char)
        let starter_template_id = "chr_0004_pelica"; // or load from config
        let starter_template = assets
            .characters
            .get(starter_template_id)
            .context("Starter character template not found")?;

        let attrs = assets
            .characters
            .get_stats(starter_template_id, 1, 0)
            .context("Starter attributes not found for level 1, break 0")?;

        let skill_levels = assets
            .char_skills
            .get_char_skills(starter_template_id)
            .into_iter()
            .filter_map(|b| b.entries.first())
            .map(|e| (e.skill_id.clone(), 1u32))
            .collect();

        let starter = Char {
            template_id: starter_template_id.to_string(),
            level: attrs.level,
            exp: 200,
            break_stage: attrs.break_stage,
            is_dead: false,
            weapon_id: WeaponIndex::default(),
            own_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or_else(|_| panic!("System clock went backwards")),
            hp: attrs.hp,
            ultimate_sp: 0.0,
            skill_levels,
        };

        let char_idx = bag.add_char(starter);

        // Create default team with starter
        let mut default_team = Team::default();
        default_team.name = "Team 1".to_string();
        default_team.char_team[0] = TeamSlot::Occupied(char_idx);
        default_team.leader_index = char_idx;

        bag.teams.push(default_team);
        bag.meta.curr_team_index = 0;

        Ok(bag)
    }

    // Find character by template ID
    pub fn char_index_by_id(&self, template_id: &str) -> Option<CharIndex> {
        self.chars
            .iter()
            .position(|c| c.template_id == template_id)
            .map(CharIndex::from_usize)
    }

    // Get character by index
    pub fn get_char(&self, idx: CharIndex) -> Option<&Char> {
        self.chars.get(idx.as_usize())
    }

    // Add a new character
    pub fn add_char(&mut self, char: Char) -> CharIndex {
        let idx = CharIndex::from_usize(self.chars.len());
        self.chars.push(char);
        idx
    }

    pub fn get_char_by_objid_mut(&mut self, objid: u64) -> Option<&mut Char> {
        let idx = CharIndex::from_object_id(objid).as_usize();
        self.chars.get_mut(idx)
    }
}
