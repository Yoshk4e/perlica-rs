use config::mission::{MissionAssets, QuestDefinition};
use perlica_proto::{
    GuideGroupInfo, Mission, MissionState, ObjectiveValueOp, Quest, QuestObjective, QuestState,
    RoleBaseInfo, ScMissionStateUpdate, ScQuestObjectivesUpdate, ScQuestStateUpdate,
    ScSyncAllGuide, ScSyncAllMission,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use tracing::error;

pub const PROLOGUE_MISSION_ID: &str = "mission_mai_e0m1";
pub const PROLOGUE_FIRST_QUEST_ID: &str = "mission_mai_e0m1_q#2";
const OBJECTIVE_PROGRESS_KEY: &str = "progress";
const GUIDE_STATE_COMPLETED: i32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MissionProgress {
    mission_id: String,
    mission_state: MissionState,
    succeed_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QuestProgress {
    quest_id: String,
    quest_state: QuestState,
    objectives: BTreeMap<String, QuestObjective>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuideManager {
    completed_groups: Vec<String>,
    completed_key_steps: Vec<String>,
}

impl GuideManager {
    pub fn mark_group_completed(&mut self, guide_group_id: &str) {
        if !self
            .completed_groups
            .iter()
            .any(|entry| entry == guide_group_id)
        {
            self.completed_groups.push(guide_group_id.to_string());
            self.completed_groups.sort();
        }
    }

    pub fn mark_key_step_completed(&mut self, guide_group_id: &str) {
        if !self
            .completed_key_steps
            .iter()
            .any(|entry| entry == guide_group_id)
        {
            self.completed_key_steps.push(guide_group_id.to_string());
            self.completed_key_steps.sort();
        }
    }

    pub fn sync_packet(&self) -> ScSyncAllGuide {
        ScSyncAllGuide {
            guide_group_list: self
                .completed_groups
                .iter()
                .cloned()
                .map(|guide_group_id| GuideGroupInfo {
                    guide_group_id,
                    guide_state: GUIDE_STATE_COMPLETED,
                })
                .collect(),
            completed_repeat_accept_guide_group_list: self.completed_key_steps.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MissionManager {
    track_mission_id: String,
    missions: BTreeMap<String, MissionProgress>,
    current_quests: BTreeMap<String, QuestProgress>,
}

#[derive(Debug, Clone, Default)]
pub struct MissionUpdate {
    pub reply_objective_update: Option<ScQuestObjectivesUpdate>,
    pub notify_objective_updates: Vec<ScQuestObjectivesUpdate>,
    pub state_updates: Vec<ScQuestStateUpdate>,
    pub mission_updates: Vec<ScMissionStateUpdate>,
}

impl MissionManager {
    pub fn ensure_bootstrap(&mut self, mission_assets: &MissionAssets) {
        if !self.missions.is_empty() {
            return;
        }

        let mission_id = if mission_assets.get(PROLOGUE_MISSION_ID).is_some() {
            PROLOGUE_MISSION_ID.to_string()
        } else {
            mission_assets
                .missions()
                .next()
                .map(|mission| mission.mission_id.clone())
                .unwrap_or_default()
        };

        if mission_id.is_empty() {
            return;
        }

        let target_quest_id = mission_assets
            .get(&mission_id)
            .and_then(|mission| mission.quests.first())
            .map(|quest| quest.quest_id.clone())
            .or_else(|| {
                (mission_id == PROLOGUE_MISSION_ID).then(|| PROLOGUE_FIRST_QUEST_ID.to_string())
            })
            .unwrap_or_else(|| format!("{mission_id}_q#1"));

        self.track_mission_id = mission_id.clone();
        self.missions.insert(
            mission_id.clone(),
            MissionProgress {
                mission_id: mission_id.clone(),
                mission_state: MissionState::Msprocessing,
                succeed_id: 0,
            },
        );

        let Some(mission) = mission_assets.get(&mission_id) else {
            error!(
                "Mission definition for '{}' not found. Cannot bootstrap quests.",
                mission_id
            );
            return;
        };

        let mut successfully_inserted = false;

        for quest in &mission.quests {
            if quest.objective_keys.is_empty() {
                error!(
                    "Quest '{}' in mission '{}' is missing objective keys. Ignoring and continuing.",
                    quest.quest_id, mission_id
                );
                continue;
            }

            if quest.quest_id == target_quest_id || !successfully_inserted {
                let quest_progress = build_quest_progress(quest);

                if !quest_progress.objectives.is_empty() {
                    self.current_quests
                        .insert(quest.quest_id.clone(), quest_progress);
                    successfully_inserted = true;
                    break; // Successfully got one and "sent" it to state, stop looking
                }
            }
        }

        if !successfully_inserted {
            error!(
                "Failed to bootstrap any valid quests for mission '{}'. All were missing or errored.",
                mission_id
            );
        }
    }

    pub fn sync_packet(&self) -> ScSyncAllMission {
        ScSyncAllMission {
            track_mission_id: self.track_mission_id.clone(),
            missions: self
                .missions
                .iter()
                .map(|(mission_id, mission)| {
                    (
                        mission_id.clone(),
                        Mission {
                            mission_id: mission.mission_id.clone(),
                            mission_state: mission.mission_state as i32,
                            succeed_id: mission.succeed_id,
                        },
                    )
                })
                .collect(),
            cur_quests: self
                .current_quests
                .iter()
                .map(|(quest_id, quest)| {
                    (
                        quest_id.clone(),
                        Quest {
                            quest_id: quest.quest_id.clone(),
                            quest_state: quest.quest_state as i32,
                            quest_objectives: quest.objectives.values().cloned().collect(),
                        },
                    )
                })
                .collect(),
        }
    }

    pub fn track_mission_id(&self) -> &str {
        &self.track_mission_id
    }

    pub fn update_track_mission(&mut self, mission_id: &str) {
        self.track_mission_id = mission_id.to_string();
    }

    pub fn stop_tracking(&mut self) {
        self.track_mission_id.clear();
    }

    pub fn apply_objective_ops(
        &mut self,
        quest_id: &str,
        objective_ops: &[ObjectiveValueOp],
        mission_assets: &MissionAssets,
        role_base_info: Option<RoleBaseInfo>,
    ) -> MissionUpdate {
        let mut update = MissionUpdate::default();
        let Some(quest) = self.current_quests.get_mut(quest_id) else {
            return update;
        };

        for op in objective_ops {
            let objective = quest
                .objectives
                .entry(op.condition_id.clone())
                .or_insert_with(|| empty_objective(&op.condition_id));

            let next_value = if op.is_add {
                objective
                    .values
                    .get(OBJECTIVE_PROGRESS_KEY)
                    .copied()
                    .unwrap_or_default()
                    .saturating_add(op.value)
            } else {
                op.value
            };

            objective
                .values
                .insert(OBJECTIVE_PROGRESS_KEY.to_string(), next_value);
            objective.is_complete = next_value > 0;
        }

        update.reply_objective_update = Some(ScQuestObjectivesUpdate {
            quest_id: quest.quest_id.clone(),
            quest_objectives: quest.objectives.values().cloned().collect(),
        });

        let is_completed = !quest.objectives.is_empty()
            && quest
                .objectives
                .values()
                .all(|objective| objective.is_complete);
        if !is_completed {
            return update;
        }

        quest.quest_state = QuestState::Qscompleted;
        update.state_updates.push(ScQuestStateUpdate {
            quest_id: quest.quest_id.clone(),
            quest_state: QuestState::Qscompleted as i32,
            role_base_info: role_base_info.clone(),
        });

        if let Some((mission_id, next_quest_id)) =
            self.advance_to_next_quest(quest_id, mission_assets)
        {
            if let Some(next_quest) = self.current_quests.get(&next_quest_id) {
                update.state_updates.push(ScQuestStateUpdate {
                    quest_id: next_quest.quest_id.clone(),
                    quest_state: QuestState::Qsprocessing as i32,
                    role_base_info: role_base_info.clone(),
                });
                update
                    .notify_objective_updates
                    .push(ScQuestObjectivesUpdate {
                        quest_id: next_quest.quest_id.clone(),
                        quest_objectives: next_quest.objectives.values().cloned().collect(),
                    });
            }

            if let Some(mission) = self.missions.get(&mission_id) {
                update.mission_updates.push(ScMissionStateUpdate {
                    mission_id: mission.mission_id.clone(),
                    mission_state: mission.mission_state as i32,
                    succeed_id: mission.succeed_id,
                    role_base_info,
                });
            }
        }

        update
    }

    fn advance_to_next_quest(
        &mut self,
        quest_id: &str,
        mission_assets: &MissionAssets,
    ) -> Option<(String, String)> {
        let mission_id = quest_id.split("_q#").next()?.to_string();
        let mission_definition = mission_assets.get(&mission_id)?;
        let current_index = mission_definition
            .quests
            .iter()
            .position(|quest| quest.quest_id == quest_id)?;

        self.current_quests.remove(quest_id);

        if let Some(next_quest) = mission_definition.quests.get(current_index + 1) {
            self.current_quests.insert(
                next_quest.quest_id.clone(),
                build_quest_progress(next_quest),
            );
            Some((mission_id, next_quest.quest_id.clone()))
        } else {
            if let Some(mission) = self.missions.get_mut(&mission_id) {
                mission.mission_state = MissionState::Mscompleted;
            }
            Some((mission_id, String::new()))
        }
    }
}

fn build_quest_progress(quest_definition: &QuestDefinition) -> QuestProgress {
    QuestProgress {
        quest_id: quest_definition.quest_id.clone(),
        quest_state: QuestState::Qsprocessing,
        objectives: quest_definition
            .objective_keys
            .iter()
            .filter_map(|objective_key| {
                if objective_key.trim().is_empty() {
                    error!(
                        "Missing or empty objective key in quest '{}'. Ignoring and continuing.",
                        quest_definition.quest_id
                    );
                    None
                } else {
                    Some((objective_key.clone(), empty_objective(objective_key)))
                    // Keep valid ones
                }
            })
            .collect(),
    }
}

fn empty_objective(condition_id: &str) -> QuestObjective {
    QuestObjective {
        condition_id: condition_id.to_string(),
        extra_details: HashMap::new(),
        is_complete: false,
        values: HashMap::from([(OBJECTIVE_PROGRESS_KEY.to_string(), 1)]),
    }
}
