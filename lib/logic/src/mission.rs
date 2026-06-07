use config::mission::{MissionAssets, MissionDefinition, MissionKind, QuestDefinition};
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

    /// Read-only accessor for the persistence layer (`perlica-db`).
    pub fn completed_groups(&self) -> &[String] {
        &self.completed_groups
    }

    /// Read-only accessor for the persistence layer (`perlica-db`).
    pub fn completed_key_steps(&self) -> &[String] {
        &self.completed_key_steps
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
                    break;
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

    /// Snapshot every persisted mission as flat tuples. Used by
    /// `perlica-db` to flush the `beyond_missions` table.
    ///
    /// Returns owned `String`s so the caller can hand them straight
    /// to `sqlx::query::bind` without juggling lifetimes against the
    /// `&self` borrow that ends as soon as this call returns.
    pub fn snapshot_missions(&self) -> Vec<(String, MissionState, i32)> {
        self.missions
            .values()
            .map(|m| (m.mission_id.clone(), m.mission_state, m.succeed_id))
            .collect()
    }

    /// Snapshot every currently active quest along with its
    /// objectives. Used by `perlica-db` to flush `beyond_quests` and
    /// `beyond_quest_objectives`.
    pub fn snapshot_quests(&self) -> Vec<(String, QuestState, Vec<QuestObjective>)> {
        self.current_quests
            .values()
            .map(|q| {
                (
                    q.quest_id.clone(),
                    q.quest_state,
                    q.objectives.values().cloned().collect(),
                )
            })
            .collect()
    }

    /// Bulk-insert a mission record at load time. Used by `perlica-db`
    /// to rehydrate the manager from the `beyond_missions` table.
    /// The mission tracker is restored separately via
    /// [`Self::update_track_mission`].
    pub fn insert_loaded_mission(
        &mut self,
        mission_id: String,
        mission_state: MissionState,
        succeed_id: i32,
    ) {
        self.missions.insert(
            mission_id.clone(),
            MissionProgress {
                mission_id,
                mission_state,
                succeed_id,
            },
        );
    }

    /// Bulk-insert a quest with all of its objectives at load time.
    /// Used by `perlica-db` to rehydrate the manager from the
    /// `beyond_quests` + `beyond_quest_objectives` tables.
    pub fn insert_loaded_quest(
        &mut self,
        quest_id: String,
        quest_state: QuestState,
        objectives: Vec<QuestObjective>,
    ) {
        let objectives_map: BTreeMap<String, QuestObjective> = objectives
            .into_iter()
            .map(|o| (o.condition_id.clone(), o))
            .collect();
        self.current_quests.insert(
            quest_id.clone(),
            QuestProgress {
                quest_id,
                quest_state,
                objectives: objectives_map,
            },
        );
    }

    pub fn has_mission(&self, mission_id: &str) -> bool {
        self.missions.contains_key(mission_id)
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
                    .get(&op.condition_id)
                    .copied()
                    .unwrap_or_default()
                    .saturating_add(op.value)
            } else {
                op.value
            };

            objective.values.insert(op.condition_id.clone(), next_value);
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

    /// Advance the *currently tracked* quest by completing its next pending
    /// objective. Used by server-authoritative progression sources such as
    /// level-script property flips and custom-event triggers (the prologue
    /// flow flags `isTimelineOver`, `isWalkLimitFinish`, …, never send a
    /// CsUpdateQuestObjective, the server has to push the step itself).
    ///
    /// Returns the same MissionUpdate shape `apply_objective_ops` produces, so
    /// the handler can re-use the existing notify plumbing.
    pub fn advance_tracked_quest_step(
        &mut self,
        mission_assets: &MissionAssets,
        role_base_info: Option<RoleBaseInfo>,
    ) -> MissionUpdate {
        // Pick the live quest belonging to the currently tracked mission.
        let track_mission = self.track_mission_id.clone();
        let Some((quest_id, condition_id)) = self
            .current_quests
            .iter()
            .find(|(qid, q)| {
                qid.starts_with(&track_mission)
                    && q.quest_state == QuestState::Qsprocessing
                    && q.objectives.values().any(|o| !o.is_complete)
            })
            .and_then(|(qid, q)| {
                q.objectives
                    .values()
                    .find(|o| !o.is_complete)
                    .map(|o| (qid.clone(), o.condition_id.clone()))
            })
        else {
            return MissionUpdate::default();
        };

        // Re-use the existing op pipeline so state/mission advancement,
        // notify packets, and follow-up quest bootstrap all stay in one place.
        let op = ObjectiveValueOp {
            condition_id,
            value: 1,
            is_add: true,
        };
        self.apply_objective_ops(&quest_id, &[op], mission_assets, role_base_info)
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
        values: HashMap::from([(condition_id.to_string(), 0)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_quest_def(quest_id: &str, objective_keys: &[&str]) -> QuestDefinition {
        QuestDefinition {
            quest_id: quest_id.to_string(),
            ordinal_key: quest_id.split("_q#").last().unwrap_or("1").to_string(),
            numeric_ordinal: quest_id.split("_q#").last().and_then(|s| s.parse().ok()),
            objective_keys: objective_keys.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn make_mission_def(mission_id: &str, quests: Vec<QuestDefinition>) -> MissionDefinition {
        MissionDefinition {
            mission_id: mission_id.to_string(),
            kind: MissionKind::from_mission_id(mission_id),
            name_key: format!("{mission_id}_name"),
            description_key: Some(format!("{mission_id}_description")),
            quests,
        }
    }

    fn make_mission_assets(missions: Vec<MissionDefinition>) -> MissionAssets {
        let map: HashMap<String, MissionDefinition> = missions
            .into_iter()
            .map(|m| (m.mission_id.clone(), m))
            .collect();
        MissionAssets::from_missions(map)
    }

    fn make_objective(condition_id: &str, value: i32, is_complete: bool) -> QuestObjective {
        QuestObjective {
            condition_id: condition_id.to_string(),
            extra_details: HashMap::new(),
            is_complete,
            values: HashMap::from([(condition_id.to_string(), value)]),
        }
    }

    #[test]
    fn guide_manager_default_is_empty() {
        let gm = GuideManager::default();
        assert!(gm.completed_groups().is_empty());
        assert!(gm.completed_key_steps().is_empty());
    }

    #[test]
    fn guide_manager_mark_group_completed_adds_group() {
        let mut gm = GuideManager::default();
        gm.mark_group_completed("guide_001");
        assert_eq!(gm.completed_groups(), &["guide_001"]);
    }

    #[test]
    fn guide_manager_mark_group_completed_no_duplicate() {
        let mut gm = GuideManager::default();
        gm.mark_group_completed("guide_001");
        gm.mark_group_completed("guide_001");
        assert_eq!(gm.completed_groups().len(), 1);
    }

    #[test]
    fn guide_manager_mark_group_completed_sorts_alphabetically() {
        let mut gm = GuideManager::default();
        gm.mark_group_completed("guide_b");
        gm.mark_group_completed("guide_a");
        gm.mark_group_completed("guide_c");
        assert_eq!(gm.completed_groups(), &["guide_a", "guide_b", "guide_c"]);
    }

    #[test]
    fn guide_manager_mark_key_step_completed_adds_step() {
        let mut gm = GuideManager::default();
        gm.mark_key_step_completed("step_001");
        assert_eq!(gm.completed_key_steps(), &["step_001"]);
    }

    #[test]
    fn guide_manager_mark_key_step_completed_no_duplicate() {
        let mut gm = GuideManager::default();
        gm.mark_key_step_completed("step_001");
        gm.mark_key_step_completed("step_001");
        assert_eq!(gm.completed_key_steps().len(), 1);
    }

    #[test]
    fn guide_manager_mark_key_step_completed_sorts_alphabetically() {
        let mut gm = GuideManager::default();
        gm.mark_key_step_completed("step_c");
        gm.mark_key_step_completed("step_a");
        gm.mark_key_step_completed("step_b");
        assert_eq!(gm.completed_key_steps(), &["step_a", "step_b", "step_c"]);
    }

    #[test]
    fn guide_manager_sync_packet_empty() {
        let gm = GuideManager::default();
        let pkt = gm.sync_packet();
        assert!(pkt.guide_group_list.is_empty());
        assert!(pkt.completed_repeat_accept_guide_group_list.is_empty());
    }

    #[test]
    fn guide_manager_sync_packet_with_data() {
        let mut gm = GuideManager::default();
        gm.mark_group_completed("g1");
        gm.mark_group_completed("g2");
        gm.mark_key_step_completed("s1");
        let pkt = gm.sync_packet();
        assert_eq!(pkt.guide_group_list.len(), 2);
        assert_eq!(pkt.guide_group_list[0].guide_group_id, "g1");
        assert_eq!(pkt.guide_group_list[0].guide_state, GUIDE_STATE_COMPLETED);
        assert_eq!(pkt.guide_group_list[1].guide_group_id, "g2");
        assert_eq!(pkt.completed_repeat_accept_guide_group_list, vec!["s1"]);
    }

    #[test]
    fn guide_manager_groups_and_steps_independent() {
        let mut gm = GuideManager::default();
        gm.mark_group_completed("group_a");
        gm.mark_key_step_completed("step_a");
        assert_eq!(gm.completed_groups().len(), 1);
        assert_eq!(gm.completed_key_steps().len(), 1);
        // They should be stored separately
        assert_eq!(gm.completed_groups()[0], "group_a");
        assert_eq!(gm.completed_key_steps()[0], "step_a");
    }

    #[test]
    fn mission_manager_default_is_empty() {
        let mgr = MissionManager::default();
        assert!(mgr.track_mission_id().is_empty());
        assert!(!mgr.has_mission("any"));
        assert!(mgr.snapshot_missions().is_empty());
        assert!(mgr.snapshot_quests().is_empty());
    }

    #[test]
    fn mission_manager_insert_loaded_mission_and_has_mission() {
        let mut mgr = MissionManager::default();
        assert!(!mgr.has_mission("mission_mai_e0m1"));
        mgr.insert_loaded_mission(
            "mission_mai_e0m1".to_string(),
            MissionState::Msprocessing,
            0,
        );
        assert!(mgr.has_mission("mission_mai_e0m1"));
        assert!(!mgr.has_mission("mission_mai_e0m2"));
    }

    #[test]
    fn mission_manager_snapshot_missions() {
        let mut mgr = MissionManager::default();
        mgr.insert_loaded_mission("m1".to_string(), MissionState::Msprocessing, 0);
        mgr.insert_loaded_mission("m2".to_string(), MissionState::Mscompleted, 1);
        let snapshot = mgr.snapshot_missions();
        assert_eq!(snapshot.len(), 2);
        // BTreeMap iteration is ordered by key
        let ids: Vec<&str> = snapshot.iter().map(|(id, _, _)| id.as_str()).collect();
        assert!(ids.contains(&"m1"));
        assert!(ids.contains(&"m2"));
    }

    #[test]
    fn mission_manager_snapshot_missions_states() {
        let mut mgr = MissionManager::default();
        mgr.insert_loaded_mission("m1".to_string(), MissionState::Msprocessing, 0);
        mgr.insert_loaded_mission("m2".to_string(), MissionState::Mscompleted, 5);
        let snapshot = mgr.snapshot_missions();
        let m1 = snapshot.iter().find(|(id, _, _)| id == "m1").unwrap();
        assert_eq!(m1.1, MissionState::Msprocessing);
        assert_eq!(m1.2, 0);
        let m2 = snapshot.iter().find(|(id, _, _)| id == "m2").unwrap();
        assert_eq!(m2.1, MissionState::Mscompleted);
        assert_eq!(m2.2, 5);
    }

    #[test]
    fn mission_manager_insert_loaded_quest_and_snapshot() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest(
            "mission_mai_e0m1_q#1".to_string(),
            QuestState::Qsprocessing,
            vec![obj],
        );
        let snapshot = mgr.snapshot_quests();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].0, "mission_mai_e0m1_q#1");
        assert_eq!(snapshot[0].1, QuestState::Qsprocessing);
        assert_eq!(snapshot[0].2.len(), 1);
    }

    #[test]
    fn mission_manager_insert_loaded_quest_multiple_objectives() {
        let mut mgr = MissionManager::default();
        let obj1 = make_objective("obj_1", 1, true);
        let obj2 = make_objective("obj_2", 0, false);
        mgr.insert_loaded_quest(
            "quest_a".to_string(),
            QuestState::Qsprocessing,
            vec![obj1, obj2],
        );
        let snapshot = mgr.snapshot_quests();
        assert_eq!(snapshot[0].2.len(), 2);
    }

    #[test]
    fn mission_manager_update_track_mission() {
        let mut mgr = MissionManager::default();
        assert!(mgr.track_mission_id().is_empty());
        mgr.update_track_mission("mission_mai_e0m1");
        assert_eq!(mgr.track_mission_id(), "mission_mai_e0m1");
        mgr.update_track_mission("mission_mai_e0m2");
        assert_eq!(mgr.track_mission_id(), "mission_mai_e0m2");
    }

    #[test]
    fn mission_manager_stop_tracking() {
        let mut mgr = MissionManager::default();
        mgr.update_track_mission("mission_mai_e0m1");
        assert_eq!(mgr.track_mission_id(), "mission_mai_e0m1");
        mgr.stop_tracking();
        assert!(mgr.track_mission_id().is_empty());
    }

    #[test]
    fn mission_manager_sync_packet_empty() {
        let mgr = MissionManager::default();
        let pkt = mgr.sync_packet();
        assert!(pkt.track_mission_id.is_empty());
        assert!(pkt.missions.is_empty());
        assert!(pkt.cur_quests.is_empty());
    }

    #[test]
    fn mission_manager_sync_packet_with_data() {
        let mut mgr = MissionManager::default();
        mgr.update_track_mission("mission_mai_e0m1");
        mgr.insert_loaded_mission(
            "mission_mai_e0m1".to_string(),
            MissionState::Msprocessing,
            0,
        );
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest(
            "mission_mai_e0m1_q#1".to_string(),
            QuestState::Qsprocessing,
            vec![obj],
        );
        let pkt = mgr.sync_packet();
        assert_eq!(pkt.track_mission_id, "mission_mai_e0m1");
        assert_eq!(pkt.missions.len(), 1);
        assert!(pkt.missions.contains_key("mission_mai_e0m1"));
        assert_eq!(
            pkt.missions["mission_mai_e0m1"].mission_state,
            MissionState::Msprocessing as i32
        );
        assert_eq!(pkt.cur_quests.len(), 1);
        assert!(pkt.cur_quests.contains_key("mission_mai_e0m1_q#1"));
    }

    #[test]
    fn mission_manager_insert_loaded_mission_replaces() {
        let mut mgr = MissionManager::default();
        mgr.insert_loaded_mission("m1".to_string(), MissionState::Msprocessing, 0);
        mgr.insert_loaded_mission("m1".to_string(), MissionState::Mscompleted, 1);
        // Should overwrite, not duplicate
        assert!(mgr.has_mission("m1"));
        let snapshot = mgr.snapshot_missions();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].1, MissionState::Mscompleted);
        assert_eq!(snapshot[0].2, 1);
    }

    #[test]
    fn ensure_bootstrap_no_op_when_missions_exist() {
        let mut mgr = MissionManager::default();
        mgr.insert_loaded_mission("existing".to_string(), MissionState::Msprocessing, 0);
        let assets = make_mission_assets(vec![]);
        mgr.ensure_bootstrap(&assets);
        // Should NOT overwrite the existing mission
        assert!(mgr.has_mission("existing"));
        assert!(!mgr.has_mission(PROLOGUE_MISSION_ID));
    }

    #[test]
    fn ensure_bootstrap_empty_assets_no_change() {
        let mut mgr = MissionManager::default();
        let assets = MissionAssets::default();
        mgr.ensure_bootstrap(&assets);
        assert!(mgr.snapshot_missions().is_empty());
        assert!(mgr.track_mission_id().is_empty());
    }

    #[test]
    fn ensure_bootstrap_prologue_mission() {
        let mut mgr = MissionManager::default();
        let quest = make_quest_def("mission_mai_e0m1_q#2", &["obj_timeline_over"]);
        let mission = make_mission_def(PROLOGUE_MISSION_ID, vec![quest]);
        let assets = make_mission_assets(vec![mission]);
        mgr.ensure_bootstrap(&assets);
        assert!(mgr.has_mission(PROLOGUE_MISSION_ID));
        assert_eq!(mgr.track_mission_id(), PROLOGUE_MISSION_ID);
        // The prologue quest should be bootstrapped
        let quests = mgr.snapshot_quests();
        assert!(!quests.is_empty());
    }

    #[test]
    fn ensure_bootstrap_prologue_uses_first_quest() {
        let mut mgr = MissionManager::default();
        let quest1 = make_quest_def("mission_mai_e0m1_q#1", &["obj_first"]);
        let quest2 = make_quest_def("mission_mai_e0m1_q#2", &["obj_second"]);
        let mission = make_mission_def(PROLOGUE_MISSION_ID, vec![quest1, quest2]);
        let assets = make_mission_assets(vec![mission]);
        mgr.ensure_bootstrap(&assets);
        // Since PROLOGUE_FIRST_QUEST_ID = "mission_mai_e0m1_q#2",
        // bootstrap should pick quest2 (or quest1 if it comes first
        // with successfully_inserted logic)
        let quests = mgr.snapshot_quests();
        assert!(!quests.is_empty());
    }

    #[test]
    fn ensure_bootstrap_falls_back_to_first_available_mission() {
        let mut mgr = MissionManager::default();
        // No prologue, but another mission exists
        let quest = make_quest_def("mission_sid_e1m1_q#1", &["obj_a"]);
        let mission = make_mission_def("mission_sid_e1m1", vec![quest]);
        let assets = make_mission_assets(vec![mission]);
        mgr.ensure_bootstrap(&assets);
        assert!(mgr.has_mission("mission_sid_e1m1"));
        assert_eq!(mgr.track_mission_id(), "mission_sid_e1m1");
    }

    #[test]
    fn ensure_bootstrap_quest_with_empty_objective_keys_is_skipped() {
        let mut mgr = MissionManager::default();
        let empty_quest = make_quest_def("mission_mai_e0m1_q#1", &[]);
        let valid_quest = make_quest_def("mission_mai_e0m1_q#2", &["obj_good"]);
        let mission = make_mission_def(PROLOGUE_MISSION_ID, vec![empty_quest, valid_quest]);
        let assets = make_mission_assets(vec![mission]);
        mgr.ensure_bootstrap(&assets);
        // Should still bootstrap a valid quest despite the empty one
        assert!(mgr.has_mission(PROLOGUE_MISSION_ID));
        let quests = mgr.snapshot_quests();
        assert!(!quests.is_empty());
        // The quest should be q#2 (the valid one), not q#1
        assert_eq!(quests[0].0, "mission_mai_e0m1_q#2");
    }

    #[test]
    fn apply_objective_ops_unknown_quest_returns_empty() {
        let mut mgr = MissionManager::default();
        let assets = MissionAssets::default();
        let ops = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 1,
            is_add: true,
        }];
        let update = mgr.apply_objective_ops("nonexistent_quest", &ops, &assets, None);
        assert!(update.reply_objective_update.is_none());
        assert!(update.state_updates.is_empty());
        assert!(update.mission_updates.is_empty());
    }

    #[test]
    fn apply_objective_ops_add_value() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest("quest_a".to_string(), QuestState::Qsprocessing, vec![obj]);
        let assets = MissionAssets::default();
        let ops = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 5,
            is_add: true,
        }];
        let update = mgr.apply_objective_ops("quest_a", &ops, &assets, None);
        assert!(update.reply_objective_update.is_some());
        let reply = update.reply_objective_update.unwrap();
        assert_eq!(reply.quest_id, "quest_a");
        assert_eq!(reply.quest_objectives.len(), 1);
        assert_eq!(reply.quest_objectives[0].values["obj_1"], 5);
        assert!(reply.quest_objectives[0].is_complete);
    }

    #[test]
    fn apply_objective_ops_set_value() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 3, true);
        mgr.insert_loaded_quest("quest_a".to_string(), QuestState::Qsprocessing, vec![obj]);
        let assets = MissionAssets::default();
        let ops = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 10,
            is_add: false,
        }];
        let update = mgr.apply_objective_ops("quest_a", &ops, &assets, None);
        let reply = update.reply_objective_update.unwrap();
        // set mode: value should be replaced, not added
        assert_eq!(reply.quest_objectives[0].values["obj_1"], 10);
    }

    #[test]
    fn apply_objective_ops_add_accumulates() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest("quest_a".to_string(), QuestState::Qsprocessing, vec![obj]);
        let assets = MissionAssets::default();
        let ops1 = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 3,
            is_add: true,
        }];
        mgr.apply_objective_ops("quest_a", &ops1, &assets, None);
        let ops2 = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 4,
            is_add: true,
        }];
        let update = mgr.apply_objective_ops("quest_a", &ops2, &assets, None);
        let reply = update.reply_objective_update.unwrap();
        assert_eq!(reply.quest_objectives[0].values["obj_1"], 7);
    }

    #[test]
    fn apply_objective_ops_new_condition_id_creates_objective() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest("quest_a".to_string(), QuestState::Qsprocessing, vec![obj]);
        let assets = MissionAssets::default();
        let ops = vec![ObjectiveValueOp {
            condition_id: "obj_new".to_string(),
            value: 1,
            is_add: true,
        }];
        let update = mgr.apply_objective_ops("quest_a", &ops, &assets, None);
        let reply = update.reply_objective_update.unwrap();
        // Should have both the original and the new objective
        assert_eq!(reply.quest_objectives.len(), 2);
    }

    #[test]
    fn apply_objective_ops_completes_quest_when_all_done() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest("quest_a".to_string(), QuestState::Qsprocessing, vec![obj]);
        let quest_def = make_quest_def("quest_a", &["obj_1"]);
        let mission = make_mission_def("mission_mai_e0m1", vec![quest_def]);
        mgr.insert_loaded_mission(
            "mission_mai_e0m1".to_string(),
            MissionState::Msprocessing,
            0,
        );
        let assets = make_mission_assets(vec![mission]);
        let ops = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 1,
            is_add: true,
        }];
        let update = mgr.apply_objective_ops("quest_a", &ops, &assets, None);
        // Quest should be completed
        assert!(!update.state_updates.is_empty());
        assert_eq!(
            update.state_updates[0].quest_state,
            QuestState::Qscompleted as i32
        );
    }

    #[test]
    fn apply_objective_ops_partial_completion_no_state_update() {
        let mut mgr = MissionManager::default();
        let obj1 = make_objective("obj_1", 0, false);
        let obj2 = make_objective("obj_2", 0, false);
        mgr.insert_loaded_quest(
            "quest_a".to_string(),
            QuestState::Qsprocessing,
            vec![obj1, obj2],
        );
        let assets = MissionAssets::default();
        let ops = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 1,
            is_add: true,
        }];
        let update = mgr.apply_objective_ops("quest_a", &ops, &assets, None);
        // Only one objective completed, quest should NOT be marked completed
        assert!(update.state_updates.is_empty());
    }

    #[test]
    fn apply_objective_ops_set_zero_marks_incomplete() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 1, true);
        mgr.insert_loaded_quest("quest_a".to_string(), QuestState::Qsprocessing, vec![obj]);
        let assets = MissionAssets::default();
        let ops = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 0,
            is_add: false,
        }];
        let update = mgr.apply_objective_ops("quest_a", &ops, &assets, None);
        let reply = update.reply_objective_update.unwrap();
        assert_eq!(reply.quest_objectives[0].values["obj_1"], 0);
        assert!(!reply.quest_objectives[0].is_complete);
        // Quest should NOT be completed
        assert!(update.state_updates.is_empty());
    }

    #[test]
    fn advance_to_next_quest_advances_quest() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest(
            "mission_mai_e0m1_q#1".to_string(),
            QuestState::Qsprocessing,
            vec![obj],
        );
        mgr.insert_loaded_mission(
            "mission_mai_e0m1".to_string(),
            MissionState::Msprocessing,
            0,
        );
        let quest1 = make_quest_def("mission_mai_e0m1_q#1", &["obj_1"]);
        let quest2 = make_quest_def("mission_mai_e0m1_q#2", &["obj_2"]);
        let mission = make_mission_def("mission_mai_e0m1", vec![quest1, quest2]);
        let assets = make_mission_assets(vec![mission]);

        let result = mgr.advance_to_next_quest("mission_mai_e0m1_q#1", &assets);
        assert!(result.is_some());
        let (mission_id, next_quest_id) = result.unwrap();
        assert_eq!(mission_id, "mission_mai_e0m1");
        assert_eq!(next_quest_id, "mission_mai_e0m1_q#2");
        // Old quest should be removed, new one inserted
        let quests = mgr.snapshot_quests();
        assert_eq!(quests.len(), 1);
        assert_eq!(quests[0].0, "mission_mai_e0m1_q#2");
    }

    #[test]
    fn advance_to_next_quest_completes_mission_when_no_more_quests() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 1, true);
        mgr.insert_loaded_quest(
            "mission_mai_e0m1_q#1".to_string(),
            QuestState::Qscompleted,
            vec![obj],
        );
        mgr.insert_loaded_mission(
            "mission_mai_e0m1".to_string(),
            MissionState::Msprocessing,
            0,
        );
        let quest1 = make_quest_def("mission_mai_e0m1_q#1", &["obj_1"]);
        let mission = make_mission_def("mission_mai_e0m1", vec![quest1]);
        let assets = make_mission_assets(vec![mission]);

        let result = mgr.advance_to_next_quest("mission_mai_e0m1_q#1", &assets);
        assert!(result.is_some());
        let (mission_id, next_quest_id) = result.unwrap();
        assert_eq!(mission_id, "mission_mai_e0m1");
        assert!(next_quest_id.is_empty()); // No next quest
        // Mission should be marked completed
        let snapshot = mgr.snapshot_missions();
        let m = snapshot
            .iter()
            .find(|(id, _, _)| id == "mission_mai_e0m1")
            .unwrap();
        assert_eq!(m.1, MissionState::Mscompleted);
    }

    #[test]
    fn advance_to_next_quest_unknown_quest_returns_none() {
        let mut mgr = MissionManager::default();
        let assets = MissionAssets::default();
        let result = mgr.advance_to_next_quest("nonexistent_q#1", &assets);
        assert!(result.is_none());
    }

    #[test]
    fn advance_tracked_quest_step_no_active_quest() {
        let mut mgr = MissionManager::default();
        let assets = MissionAssets::default();
        let update = mgr.advance_tracked_quest_step(&assets, None);
        assert!(update.reply_objective_update.is_none());
        assert!(update.state_updates.is_empty());
    }

    #[test]
    fn advance_tracked_quest_step_completes_objective() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest(
            "mission_mai_e0m1_q#1".to_string(),
            QuestState::Qsprocessing,
            vec![obj],
        );
        mgr.insert_loaded_mission(
            "mission_mai_e0m1".to_string(),
            MissionState::Msprocessing,
            0,
        );
        mgr.update_track_mission("mission_mai_e0m1");

        let quest1 = make_quest_def("mission_mai_e0m1_q#1", &["obj_1"]);
        let mission = make_mission_def("mission_mai_e0m1", vec![quest1]);
        let assets = make_mission_assets(vec![mission]);

        let update = mgr.advance_tracked_quest_step(&assets, None);
        assert!(update.reply_objective_update.is_some());
        let reply = update.reply_objective_update.unwrap();
        assert_eq!(reply.quest_objectives[0].values["obj_1"], 1);
        assert!(reply.quest_objectives[0].is_complete);
    }

    #[test]
    fn advance_tracked_quest_step_skips_completed_quest() {
        let mut mgr = MissionManager::default();
        // Quest is already completed, so no incomplete objectives to advance
        let obj = make_objective("obj_1", 1, true);
        mgr.insert_loaded_quest(
            "mission_mai_e0m1_q#1".to_string(),
            QuestState::Qscompleted,
            vec![obj],
        );
        mgr.update_track_mission("mission_mai_e0m1");
        let assets = MissionAssets::default();

        let update = mgr.advance_tracked_quest_step(&assets, None);
        // No incomplete objective, should return empty update
        assert!(update.reply_objective_update.is_none());
    }

    #[test]
    fn apply_objective_ops_advances_to_next_quest() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest(
            "mission_mai_e0m1_q#1".to_string(),
            QuestState::Qsprocessing,
            vec![obj],
        );
        mgr.insert_loaded_mission(
            "mission_mai_e0m1".to_string(),
            MissionState::Msprocessing,
            0,
        );

        let quest1 = make_quest_def("mission_mai_e0m1_q#1", &["obj_1"]);
        let quest2 = make_quest_def("mission_mai_e0m1_q#2", &["obj_2"]);
        let mission = make_mission_def("mission_mai_e0m1", vec![quest1, quest2]);
        let assets = make_mission_assets(vec![mission]);

        let ops = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 1,
            is_add: true,
        }];
        let update = mgr.apply_objective_ops("mission_mai_e0m1_q#1", &ops, &assets, None);

        // Quest should be completed
        assert!(!update.state_updates.is_empty());
        // Should have state update for the next quest
        let has_next_quest_update = update
            .state_updates
            .iter()
            .any(|su| su.quest_id == "mission_mai_e0m1_q#2");
        assert!(has_next_quest_update);
        // Should have mission update
        assert!(!update.mission_updates.is_empty());
    }

    #[test]
    fn apply_objective_ops_completes_mission_when_last_quest_done() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest(
            "mission_mai_e0m1_q#1".to_string(),
            QuestState::Qsprocessing,
            vec![obj],
        );
        mgr.insert_loaded_mission(
            "mission_mai_e0m1".to_string(),
            MissionState::Msprocessing,
            0,
        );

        let quest1 = make_quest_def("mission_mai_e0m1_q#1", &["obj_1"]);
        let mission = make_mission_def("mission_mai_e0m1", vec![quest1]);
        let assets = make_mission_assets(vec![mission]);

        let ops = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 1,
            is_add: true,
        }];
        let update = mgr.apply_objective_ops("mission_mai_e0m1_q#1", &ops, &assets, None);

        // Mission should be completed (no more quests)
        assert!(!update.mission_updates.is_empty());
        assert_eq!(
            update.mission_updates[0].mission_state,
            MissionState::Mscompleted as i32
        );
    }

    #[test]
    fn apply_objective_ops_with_role_base_info() {
        let mut mgr = MissionManager::default();
        let obj = make_objective("obj_1", 0, false);
        mgr.insert_loaded_quest("quest_a".to_string(), QuestState::Qsprocessing, vec![obj]);
        let quest_def = make_quest_def("quest_a", &["obj_1"]);
        let mission = make_mission_def("mission_mai_e0m1", vec![quest_def]);
        mgr.insert_loaded_mission(
            "mission_mai_e0m1".to_string(),
            MissionState::Msprocessing,
            0,
        );
        let assets = make_mission_assets(vec![mission]);

        let role_info = RoleBaseInfo {
            leader_char_id: 42,
            leader_position: None,
            leader_rotation: None,
            scene_name: "map01".to_string(),
            server_ts: 12345,
        };
        let ops = vec![ObjectiveValueOp {
            condition_id: "obj_1".to_string(),
            value: 1,
            is_add: true,
        }];
        let update = mgr.apply_objective_ops("quest_a", &ops, &assets, Some(role_info.clone()));
        // State updates should carry the role_base_info
        assert!(!update.state_updates.is_empty());
        assert!(update.state_updates[0].role_base_info.is_some());
        assert_eq!(
            update.state_updates[0]
                .role_base_info
                .as_ref()
                .unwrap()
                .leader_char_id,
            42
        );
    }

    #[test]
    fn empty_objective_structure() {
        let obj = empty_objective("test_cond");
        assert_eq!(obj.condition_id, "test_cond");
        assert!(!obj.is_complete);
        assert_eq!(obj.values["test_cond"], 0);
        assert!(obj.extra_details.is_empty());
    }

    #[test]
    fn mission_update_default() {
        let update = MissionUpdate::default();
        assert!(update.reply_objective_update.is_none());
        assert!(update.notify_objective_updates.is_empty());
        assert!(update.state_updates.is_empty());
        assert!(update.mission_updates.is_empty());
    }

    #[test]
    fn prologue_mission_id_constant() {
        assert_eq!(PROLOGUE_MISSION_ID, "mission_mai_e0m1");
    }

    #[test]
    fn prologue_first_quest_id_constant() {
        assert_eq!(PROLOGUE_FIRST_QUEST_ID, "mission_mai_e0m1_q#2");
    }

    #[test]
    fn guide_state_completed_constant() {
        assert_eq!(GUIDE_STATE_COMPLETED, 3);
    }
}
