use crate::error::{ConfigError, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissionKind {
    Main,
    Bloc,
    Side,
    Dungeon,
    Factory,
    Gameplay,
    Unknown(String),
}

impl MissionKind {
    pub fn from_mission_id(mission_id: &str) -> Self {
        match mission_id.split('_').nth(1) {
            Some("mai") => Self::Main,
            Some("blc") => Self::Bloc,
            Some("sid") => Self::Side,
            Some("dgn") => Self::Dungeon,
            Some("fac") => Self::Factory,
            Some("gpl") => Self::Gameplay,
            Some(other) => Self::Unknown(other.to_string()),
            None => Self::Unknown(String::new()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDefinition {
    pub quest_id: String,
    pub ordinal_key: String,
    pub numeric_ordinal: Option<u32>,
    pub objective_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionDefinition {
    pub mission_id: String,
    pub kind: MissionKind,
    pub name_key: String,
    pub description_key: Option<String>,
    pub quests: Vec<QuestDefinition>,
}

#[derive(Debug, Clone, Default)]
pub struct MissionAssets {
    missions: HashMap<String, MissionDefinition>,
}

impl MissionAssets {
    /// Construct a `MissionAssets` from a pre-built map.  Primarily
    /// intended for unit tests that need specific mission definitions
    /// without loading from disk.
    pub fn from_missions(missions: HashMap<String, MissionDefinition>) -> Self {
        Self { missions }
    }
}

impl MissionAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let key_catalog = collect_mission_keys(tables_dir)?;
        if key_catalog.is_empty() {
            return Ok(Self::default());
        }

        let mut missions = HashMap::<String, MissionDefinition>::new();

        for key in key_catalog {
            if let Some(mission_id) = key.strip_suffix("_name") {
                if !mission_id.starts_with("mission_") {
                    continue;
                }

                missions
                    .entry(mission_id.to_string())
                    .or_insert_with(|| MissionDefinition {
                        mission_id: mission_id.to_string(),
                        kind: MissionKind::from_mission_id(mission_id),
                        name_key: key.clone(),
                        description_key: None,
                        quests: Vec::new(),
                    });
                continue;
            }

            if let Some(mission_id) = key.strip_suffix("_description").filter(|mission_id| {
                mission_id.starts_with("mission_") && !mission_id.contains("_q#")
            }) {
                missions
                    .entry(mission_id.to_string())
                    .or_insert_with(|| MissionDefinition {
                        mission_id: mission_id.to_string(),
                        kind: MissionKind::from_mission_id(mission_id),
                        name_key: format!("{mission_id}_name"),
                        description_key: Some(key.clone()),
                        quests: Vec::new(),
                    })
                    .description_key = Some(key.clone());
                continue;
            }

            let Some((mission_id, quest_definition)) = parse_quest_key(&key) else {
                continue;
            };

            let mission = missions
                .entry(mission_id.clone())
                .or_insert_with(|| MissionDefinition {
                    mission_id: mission_id.clone(),
                    kind: MissionKind::from_mission_id(&mission_id),
                    name_key: format!("{mission_id}_name"),
                    description_key: Some(format!("{mission_id}_description")),
                    quests: Vec::new(),
                });

            match mission
                .quests
                .iter_mut()
                .find(|quest| quest.quest_id == quest_definition.quest_id)
            {
                Some(existing) => {
                    for key in &quest_definition.objective_keys {
                        if !existing.objective_keys.contains(key) {
                            existing.objective_keys.push(key.clone());
                        }
                    }
                }
                None => mission.quests.push(quest_definition),
            }
        }

        for mission in missions.values_mut() {
            mission.quests.sort_by(|left, right| {
                left.numeric_ordinal
                    .cmp(&right.numeric_ordinal)
                    .then_with(|| left.ordinal_key.cmp(&right.ordinal_key))
                    .then_with(|| left.quest_id.cmp(&right.quest_id))
            });

            for quest in &mut mission.quests {
                quest.objective_keys.sort();
                quest.objective_keys.dedup();
            }
        }

        Ok(Self { missions })
    }

    pub fn get(&self, mission_id: &str) -> Option<&MissionDefinition> {
        self.missions.get(mission_id)
    }

    pub fn missions(&self) -> impl Iterator<Item = &MissionDefinition> {
        self.missions.values()
    }

    pub fn is_empty(&self) -> bool {
        self.missions.is_empty()
    }
}

fn collect_mission_keys(tables_dir: &Path) -> Result<HashSet<String>> {
    let text_table_path = tables_dir.join("TextTable.json");
    if !text_table_path.exists() {
        warn!(
            "mission catalog source '{}' was not found; mission sync will be empty",
            text_table_path.display()
        );
        return Ok(HashSet::new());
    }

    let contents =
        std::fs::read_to_string(&text_table_path).map_err(|source| ConfigError::ReadFile {
            path: text_table_path.clone(),
            source,
        })?;
    let entries: HashMap<String, Value> =
        serde_json::from_str(&contents).map_err(|source| ConfigError::ParseJson {
            path: text_table_path.clone(),
            source,
        })?;

    let mut keys = HashSet::with_capacity(entries.len() / 4);
    for (key, value) in &entries {
        if key.starts_with("mission_") {
            keys.insert(key.clone());
        }

        if let Some(text_id) = value
            .get("text")
            .and_then(|text| text.get("id"))
            .and_then(|id| id.as_str())
            .filter(|&id| id.starts_with("mission_"))
        {
            keys.insert(text_id.to_string());
        }
    }

    for entry in std::fs::read_dir(tables_dir).map_err(|source| ConfigError::ReadDir {
        path: tables_dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(ConfigError::Io)?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if !file_name.starts_with("I18nTextTable_")
            || path.extension().and_then(|ext| ext.to_str()) != Some("json")
        {
            continue;
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                warn!("failed to read i18n table '{}': {}", path.display(), error);
                continue;
            }
        };

        let i18n_entries: HashMap<String, Value> = match serde_json::from_str(&contents) {
            Ok(map) => map,
            Err(error) => {
                warn!("failed to parse i18n table '{}': {}", path.display(), error);
                continue;
            }
        };

        for key in i18n_entries.keys() {
            if key.starts_with("mission_") {
                keys.insert(key.clone());
            }
        }
    }

    Ok(keys)
}

fn parse_quest_key(key: &str) -> Option<(String, QuestDefinition)> {
    if !key.starts_with("mission_") || !key.ends_with("_description") || !key.contains("_obj_") {
        return None;
    }

    let (mission_prefix, tail) = key.split_once("_q#")?;
    let (ordinal_text, _) = tail.split_once("_obj_")?;
    let mission_id = mission_prefix.to_string();
    let quest_id = format!("{mission_prefix}_q#{ordinal_text}");

    // The conditionId in the packet is the ordinal (e.g. "2", "3" for e0m1)
    let condition_id = ordinal_text.to_string();

    Some((
        mission_id,
        QuestDefinition {
            quest_id,
            ordinal_key: ordinal_text.to_string(),
            numeric_ordinal: ordinal_text.parse::<u32>().ok(),
            objective_keys: vec![condition_id],
        },
    ))
}
