use crate::scene::lv_props_to_map;
use config::BeyondAssets;
use config::tables::level_data::LvLevelScript;
use perlica_proto::{
    DynamicParameter, LevelScriptInfo, MissionState, QuestState, ScSceneLevelScriptStateNotify,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LevelScriptState {
    None = 0,
    Disabled = 1,
    #[default]
    Enabled = 2,
    Active = 3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TriggerKind {
    ScriptStart,
    ScriptActive,
    CustomEvent(String),
    GuideGroupComplete,
    ServerDialogExit,
    QuestStateChanged {
        quest_id: Option<String>,
        new_state: Option<QuestState>,
    },
    MissionStateChanged {
        mission_id: Option<String>,
        new_state: Option<MissionState>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ScriptRuntimeState {
    state: LevelScriptState,
    properties: HashMap<String, DynamicParameter>,
    committed_cache_steps: u32,
}

#[derive(Debug, Clone)]
struct ScriptTriggerSet {
    initial_state: LevelScriptState,
    triggers: Vec<TriggerKind>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LevelScriptManager {
    current_scene: String,
    runtime_by_scene: BTreeMap<String, BTreeMap<i32, ScriptRuntimeState>>,
}

impl LevelScriptManager {
    pub fn sync_scene(&mut self, scene_name: &str, assets: &BeyondAssets) {
        self.current_scene = scene_name.to_string();
        let scene_runtime = self
            .runtime_by_scene
            .entry(scene_name.to_string())
            .or_default();

        for script in assets.level_data.level_scripts(scene_name) {
            let script_id = script.script_id as i32;
            let initial_state = script_initial_state(script, scene_name);
            scene_runtime
                .entry(script_id)
                .and_modify(|runtime| {
                    if runtime.properties.is_empty() {
                        runtime.properties = lv_props_to_map(&script.properties);
                    }
                    if runtime.state == LevelScriptState::None {
                        runtime.state = initial_state;
                    }
                })
                .or_insert_with(|| ScriptRuntimeState {
                    state: initial_state,
                    properties: lv_props_to_map(&script.properties),
                    committed_cache_steps: 0,
                });
        }
    }

    pub fn reset_scene(&mut self, scene_name: &str, assets: &BeyondAssets) {
        self.runtime_by_scene.remove(scene_name);
        self.sync_scene(scene_name, assets);
    }

    pub fn packed_level_scripts(
        &self,
        scene_name: &str,
        assets: &BeyondAssets,
    ) -> Vec<LevelScriptInfo> {
        assets
            .level_data
            .level_scripts(scene_name)
            .iter()
            .map(|script| {
                let script_id = script.script_id as i32;
                let runtime = self
                    .runtime_by_scene
                    .get(scene_name)
                    .and_then(|scene| scene.get(&script_id))
                    .cloned()
                    .unwrap_or_else(|| ScriptRuntimeState {
                        state: script_initial_state(script, scene_name),
                        properties: lv_props_to_map(&script.properties),
                        committed_cache_steps: 0,
                    });

                LevelScriptInfo {
                    script_id,
                    state: runtime.state as i32,
                    properties: runtime.properties,
                }
            })
            .collect()
    }

    pub fn set_client_active(
        &mut self,
        scene_name: &str,
        script_id: i32,
        is_active: bool,
        assets: &BeyondAssets,
    ) -> Option<LevelScriptState> {
        self.sync_scene(scene_name, assets);
        let next_state = if is_active {
            LevelScriptState::Active
        } else {
            LevelScriptState::Enabled
        };
        self.set_state(scene_name, script_id, next_state)
    }

    pub fn update_properties(
        &mut self,
        scene_name: &str,
        script_id: i32,
        properties: &HashMap<String, DynamicParameter>,
        assets: &BeyondAssets,
    ) {
        self.sync_scene(scene_name, assets);
        let scene = self
            .runtime_by_scene
            .entry(scene_name.to_string())
            .or_default();
        let runtime = scene.entry(script_id).or_default();
        runtime.properties.extend(properties.clone());
    }

    pub fn commit_cache_step(
        &mut self,
        scene_name: &str,
        script_id: i32,
        assets: &BeyondAssets,
    ) -> Option<LevelScriptState> {
        self.sync_scene(scene_name, assets);
        let scene = self
            .runtime_by_scene
            .entry(scene_name.to_string())
            .or_default();
        let runtime = scene.entry(script_id).or_default();
        runtime.committed_cache_steps = runtime.committed_cache_steps.saturating_add(1);
        self.set_state(scene_name, script_id, LevelScriptState::Enabled)
    }

    pub fn on_custom_event(
        &mut self,
        scene_name: &str,
        event_name: &str,
        assets: &BeyondAssets,
    ) -> Vec<i32> {
        self.matching_scripts(scene_name, assets, |trigger| {
            matches!(trigger, TriggerKind::CustomEvent(expected) if expected == event_name)
        })
    }

    pub fn on_dialog_finished(&mut self, scene_name: &str, assets: &BeyondAssets) -> Vec<i32> {
        self.matching_scripts(scene_name, assets, |trigger| {
            matches!(trigger, TriggerKind::ServerDialogExit)
        })
    }

    pub fn on_guide_group_completed(
        &mut self,
        scene_name: &str,
        assets: &BeyondAssets,
    ) -> Vec<i32> {
        self.matching_scripts(scene_name, assets, |trigger| {
            matches!(trigger, TriggerKind::GuideGroupComplete)
        })
    }

    pub fn on_quest_state_changed(
        &mut self,
        scene_name: &str,
        quest_id: &str,
        new_state: QuestState,
        assets: &BeyondAssets,
    ) -> Vec<i32> {
        self.matching_scripts(scene_name, assets, |trigger| {
            matches!(
                trigger,
                TriggerKind::QuestStateChanged {
                    quest_id: Some(expected_quest_id),
                    new_state: Some(expected_state),
                } if expected_quest_id == quest_id && *expected_state == new_state
            )
        })
    }

    pub fn on_mission_state_changed(
        &mut self,
        scene_name: &str,
        mission_id: &str,
        new_state: MissionState,
        assets: &BeyondAssets,
    ) -> Vec<i32> {
        self.matching_scripts(scene_name, assets, |trigger| {
            matches!(
                trigger,
                TriggerKind::MissionStateChanged {
                    mission_id: Some(expected_mission_id),
                    new_state: Some(expected_state),
                } if expected_mission_id == mission_id && *expected_state == new_state
            )
        })
    }

    pub fn state_notify(
        &self,
        scene_name: &str,
        script_id: i32,
    ) -> Option<ScSceneLevelScriptStateNotify> {
        let runtime = self.runtime_by_scene.get(scene_name)?.get(&script_id)?;
        Some(ScSceneLevelScriptStateNotify {
            scene_name: scene_name.to_string(),
            script_id,
            state: runtime.state as i32,
        })
    }

    fn matching_scripts(
        &mut self,
        scene_name: &str,
        assets: &BeyondAssets,
        predicate: impl Fn(&TriggerKind) -> bool,
    ) -> Vec<i32> {
        self.sync_scene(scene_name, assets);
        let mut activated = BTreeSet::new();
        for script in assets.level_data.level_scripts(scene_name) {
            let metadata = build_trigger_set(script);
            if metadata.triggers.iter().any(&predicate) {
                let script_id = script.script_id as i32;
                self.set_state(scene_name, script_id, LevelScriptState::Active);
                activated.insert(script_id);
            }
        }
        activated.into_iter().collect()
    }

    fn set_state(
        &mut self,
        scene_name: &str,
        script_id: i32,
        next_state: LevelScriptState,
    ) -> Option<LevelScriptState> {
        let runtime = self
            .runtime_by_scene
            .entry(scene_name.to_string())
            .or_default()
            .entry(script_id)
            .or_default();

        if runtime.state == next_state {
            return None;
        }

        runtime.state = next_state;
        Some(next_state)
    }
}

fn script_initial_state(script: &LvLevelScript, scene_name: &str) -> LevelScriptState {
    let script_id = script.script_id as i32;
    match (scene_name, script_id) {
        ("map01_dg003", 5) => LevelScriptState::Active,
        ("map01_dg003", 19) => LevelScriptState::Active,
        ("map01_lv001", 70001) => LevelScriptState::Active,
        ("map01_lv001", 70010) => LevelScriptState::Active,
        ("map01_lv001", 30018) => LevelScriptState::Active,
        _ => LevelScriptState::Enabled,
    }
    // as for why we are not using build_trigger_set refer to the comment in the first match arm of it
}

fn build_trigger_set(script: &LvLevelScript) -> ScriptTriggerSet {
    let mut triggers = Vec::new();
    let mut initial_state = if script.allow_tick {
        LevelScriptState::Active
    } else {
        LevelScriptState::Enabled
    };

    let parsed_json = script
        .embedded_action_map
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
    if let Some(headers) = parsed_json.as_ref().and_then(|json| {
        json.get("dataMap")
            .and_then(|dm| dm.get("headerList"))
            .and_then(|hl| hl.as_array())
    }) {
        for header in headers {
            let Some(trigger_type) = header
                .get("$type")
                .and_then(|v| v.as_str())
                .map(short_trigger_name)
            else {
                continue;
            };

            match trigger_type {
                "OnScriptStart" => {
                    // Only auto-start if this header IS the root node of the
                    // action graph (_ID == 0).  When _ID > 0 the OnScriptStart
                    // handler sits inside a sequence driven by something else
                    // (zone entry, another script, etc.) — blindly setting
                    // Active there fires tutorials before the player arrives.
                    // Though, they still do get activated so we're not using this for the time being
                    let is_root = header
                        .get("_ID")
                        .and_then(|v| v.as_i64())
                        .map(|id| id == 0)
                        .unwrap_or(false);
                    if is_root {
                        initial_state = LevelScriptState::Active;
                    }
                    triggers.push(TriggerKind::ScriptStart);
                }
                "OnScriptActive" => {
                    initial_state = LevelScriptState::Enabled;
                    triggers.push(TriggerKind::ScriptActive);
                }
                "OnCustomEvent" => {
                    let event_name = header
                        .get("_eventKey")
                        .and_then(|v| v.get("constValue"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();

                    if !event_name.is_empty() {
                        triggers.push(TriggerKind::CustomEvent(event_name));
                    }
                }
                "OnGuideGroupComplete" => triggers.push(TriggerKind::GuideGroupComplete),
                "OnServerDialogExit" => triggers.push(TriggerKind::ServerDialogExit),
                "OnQuestStateChanged" => triggers.push(TriggerKind::QuestStateChanged {
                    quest_id: header
                        .get("_filtedQuestId")
                        .and_then(|v| v.get("constValue"))
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string),
                    new_state: header
                        .get("_filtedNewState")
                        .and_then(|v| v.get("constValue"))
                        .and_then(|v| v.as_i64())
                        .and_then(|v| QuestState::try_from(v as i32).ok()),
                }),
                "OnMissionStateChanged" => triggers.push(TriggerKind::MissionStateChanged {
                    mission_id: header
                        .get("_filtedMissionId")
                        .and_then(|v| v.get("constValue"))
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string),
                    new_state: header
                        .get("_filtedNewState")
                        .and_then(|v| v.get("constValue"))
                        .and_then(|v| v.as_i64())
                        .and_then(|v| MissionState::try_from(v as i32).ok()),
                }),
                _ => {}
            }
        }
    }

    // theoretically Spatial-trigger scripts must always start Enabled, regardless of what
    // the trigger header analysis decided.  The client owns shape-entry
    // detection and tells us via CsSceneSetLevelScriptActive when a script
    // should become Active.  A script whose *only* activation path is an
    // OnScriptStart header with no shapes is the one true case that starts
    // Active immediately, but the moment start_shapes OR active_shapes are
    // present the client is in charge and Active is wrong as an initial state.
    if !script.start_shapes.is_empty() || !script.active_shapes.is_empty() {
        initial_state = LevelScriptState::Enabled;
    }

    ScriptTriggerSet {
        initial_state,
        triggers,
    }
}

fn short_trigger_name(trigger_type: &str) -> &str {
    trigger_type
        .split(',')
        .next()
        .unwrap_or(trigger_type)
        .rsplit('.')
        .next()
        .unwrap_or(trigger_type)
}
