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
    consumed_progression_flags: BTreeSet<String>,
    consumed_server_events: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct ScriptTriggerSet {
    #[allow(dead_code)]
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
                    consumed_progression_flags: BTreeSet::new(),
                    consumed_server_events: BTreeSet::new(),
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
                        consumed_progression_flags: BTreeSet::new(),
                        consumed_server_events: BTreeSet::new(),
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

    /// Mark a progression flag as consumed for this session.
    ///
    /// Returns `true` if the flag was **not** previously consumed, the caller
    /// should advance the quest one step.  Returns `false` if the flag was
    /// already consumed, skip advancement.  This prevents:
    ///
    /// * Reconnect replays: client re-sends the same property packet.
    /// * Same-chain cascades: two `SetBool` actions in a single action-graph
    ///   chain both set progression flags, generating two property-update
    ///   packets for one player event (e.g. `isWalkLimitFinish` -> `is_G_Ob_Over`).
    pub fn try_consume_progression_flag(
        &mut self,
        scene_name: &str,
        script_id: i32,
        flag_key: &str,
    ) -> bool {
        let scene = self
            .runtime_by_scene
            .entry(scene_name.to_string())
            .or_default();
        let runtime = scene.entry(script_id).or_default();
        // BTreeSet::insert returns true iff the value was not already present.
        runtime
            .consumed_progression_flags
            .insert(flag_key.to_string())
    }

    /// Guard against duplicate `TriggerServerEvent` packets for the same
    /// (script_id, event_name) pair.
    ///
    /// Returns `true` on the **first** call (caller should process), `false`
    /// on subsequent calls (caller should skip).
    pub fn try_consume_server_event(
        &mut self,
        scene_name: &str,
        script_id: i32,
        event_name: &str,
    ) -> bool {
        let scene = self
            .runtime_by_scene
            .entry(scene_name.to_string())
            .or_default();
        let runtime = scene.entry(script_id).or_default();
        runtime
            .consumed_server_events
            .insert(event_name.to_string())
    }

    /// Scan every `Enabled` script in the scene and activate any that:
    ///
    /// 1. Have at least one `startShape` whose bounding sphere contains
    ///    `player_pos` - i.e. the player is inside the activation zone but no
    ///    proximity-entry edge fired (already inside when eligible).
    /// 2. Have an `OnScriptStart` header at root position (`_ID = 0`).
    /// 3. Pass their `OnScriptStart` validate condition given the script's
    ///    currently-stored properties (prevents re-running scripts that have
    ///    already set their `isOver` flag).
    ///
    /// Returns the list of script IDs that were newly set to `Active`.
    ///
    /// Call this after any server-side scene event (`TriggerServerEvent`,
    /// script deactivation, quest-state change) so that companion scripts
    /// whose proximity edge was missed are caught without any hardcoded
    /// event-name -> script-ID mapping.
    pub fn activate_eligible_proximate_scripts(
        &mut self,
        scene_name: &str,
        player_pos: (f32, f32, f32),
        assets: &BeyondAssets,
    ) -> Vec<i32> {
        self.sync_scene(scene_name, assets);
        let mut newly_activated = Vec::new();

        for script in assets.level_data.level_scripts(scene_name) {
            let script_id = script.script_id as i32;

            // Only consider scripts that are Enabled (not Active, not Disabled).
            if self.get_state(scene_name, script_id) != LevelScriptState::Enabled {
                continue;
            }

            // Must have at least one startShape that contains the player.
            // In activate_eligible_proximate_scripts, replace the in_zone check:
            let in_zone = script
                .start_shapes
                .iter()
                .any(|shape| shape_contains_point(shape, player_pos))
                || script
                    .active_shapes
                    .iter()
                    .any(|shape| shape_contains_point(shape, player_pos));
            if !in_zone {
                continue;
            }

            // Parse the action map once to check OnScriptStart eligibility.
            let Some(am_str) = script.embedded_action_map.as_deref() else {
                continue;
            };
            let Ok(am) = serde_json::from_str::<serde_json::Value>(am_str) else {
                continue;
            };
            let Some(data_map) = am.get("dataMap") else {
                continue;
            };
            let headers = data_map
                .get("headerList")
                .and_then(|v| v.as_array())
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let getters = data_map
                .get("getterList")
                .and_then(|v| v.as_array())
                .map(Vec::as_slice)
                .unwrap_or(&[]);

            if !has_root_on_script_start(headers) {
                continue;
            }

            // Evaluate the OnScriptStart validate against stored properties.
            let stored_props = self
                .runtime_by_scene
                .get(scene_name)
                .and_then(|s| s.get(&script_id))
                .map(|r| &r.properties);

            if !on_script_start_validate_passes(headers, getters, stored_props) {
                continue;
            }

            if self
                .set_state(scene_name, script_id, LevelScriptState::Active)
                .is_some()
            {
                newly_activated.push(script_id);
            }
        }

        newly_activated
    }

    pub fn activate_server_triggered_scripts(
        &mut self,
        scene_name: &str,
        assets: &BeyondAssets,
    ) -> Vec<i32> {
        self.sync_scene(scene_name, assets);
        let mut newly_activated = Vec::new();

        for script in assets.level_data.level_scripts(scene_name) {
            let script_id = script.script_id as i32;

            if self.get_state(scene_name, script_id) != LevelScriptState::Enabled {
                continue;
            }

            // Only shape-less scripts, spatial ones are handled by the client
            // or by activate_eligible_proximate_scripts.
            if !script.start_shapes.is_empty() || !script.active_shapes.is_empty() {
                continue;
            }

            let Some(am_str) = script.embedded_action_map.as_deref() else {
                continue;
            };
            let Ok(am) = serde_json::from_str::<serde_json::Value>(am_str) else {
                continue;
            };
            let Some(data_map) = am.get("dataMap") else {
                continue;
            };
            let headers = data_map
                .get("headerList")
                .and_then(|v| v.as_array())
                .map(Vec::as_slice)
                .unwrap_or(&[]);

            // Must have OnScriptActive at root (_ID = 0).
            let has_on_active = headers.iter().any(|h| {
                short_trigger_name(h.get("$type").and_then(|v| v.as_str()).unwrap_or_default())
                    == "OnScriptActive"
                    && h.get("_ID").and_then(|v| v.as_i64()) == Some(0)
            });
            if !has_on_active {
                continue;
            }

            // Skip scripts that have already completed (isOver = true).
            let stored_props = self
                .runtime_by_scene
                .get(scene_name)
                .and_then(|s| s.get(&script_id))
                .map(|r| &r.properties);
            if let Some(props) = stored_props {
                if let Some(is_over) = props.get("isOver") {
                    if is_over.value_bool_list.first().copied().unwrap_or(false) {
                        continue;
                    }
                }
            }

            if self
                .set_state(scene_name, script_id, LevelScriptState::Active)
                .is_some()
            {
                newly_activated.push(script_id);
            }
        }

        newly_activated
    }

    fn get_state(&self, scene_name: &str, script_id: i32) -> LevelScriptState {
        self.runtime_by_scene
            .get(scene_name)
            .and_then(|s| s.get(&script_id))
            .map(|r| r.state)
            .unwrap_or_default()
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
                    // (zone entry, another script, etc.) - blindly setting
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

/// Point-in-zone test using the shape's `radius` field as a bounding sphere.
///
/// We use `radius` regardless of `shape_type` because:
/// - It is always present and represents the shape's outer extent.
/// - For proximity-activation purposes an inclusive sphere is correct: if the
///   player is within `radius` units of the shape centre they are "in the
///   zone".  This may be slightly generous for OBB shapes, but it is never
///   incorrectly exclusive.
fn shape_contains_point(shape: &config::tables::level_data::LvShape, pos: (f32, f32, f32)) -> bool {
    let dx = pos.0 - shape.offset.x;
    let dy = pos.1 - shape.offset.y;
    let dz = pos.2 - shape.offset.z;
    dx * dx + dy * dy + dz * dz <= shape.radius * shape.radius
}

/// Returns `true` if the script's root `OnScriptStart` header has no validate
/// condition, or if the validate evaluates to `true` given `stored_props`.
///
/// Only called for scripts where `has_root_on_script_start` already returned
/// `true`, so the header is guaranteed to exist.
fn on_script_start_validate_passes(
    headers: &[serde_json::Value],
    getters: &[serde_json::Value],
    stored_props: Option<&std::collections::HashMap<String, DynamicParameter>>,
) -> bool {
    let Some(header) = headers.iter().find(|h| {
        short_trigger_name(h.get("$type").and_then(|v| v.as_str()).unwrap_or_default())
            == "OnScriptStart"
            && h.get("_ID").and_then(|v| v.as_i64()) == Some(0)
    }) else {
        return true;
    };

    let Some(validate) = header.get("_validate") else {
        return true;
    };
    let Some(id_ref) = validate.get("idRef").and_then(|v| v.as_i64()) else {
        return true;
    };

    eval_getter(id_ref as i32, getters, stored_props)
}

/// Returns `true` if there is an `OnScriptStart` header at root position
/// (`_ID = 0`), meaning the script can start without any prior sequencing.
fn has_root_on_script_start(headers: &[serde_json::Value]) -> bool {
    headers.iter().any(|h| {
        short_trigger_name(h.get("$type").and_then(|v| v.as_str()).unwrap_or_default())
            == "OnScriptStart"
            && h.get("_ID").and_then(|v| v.as_i64()) == Some(0)
    })
}

/// Evaluate a getter by its `_ID`, returning the boolean result.
///
/// Handles the two patterns seen across all map01_dg003 scripts:
///
/// | Getter type        | Meaning                                              |
/// |--------------------|------------------------------------------------------|
/// | `BoolGetter`       | Reads `self_bb.<key>` from stored properties         |
/// | `BoolGetterInvert` | Negates an inner getter identified by explicit idRef |
/// |                    | (or by `_ID - 1` when paramSource = -1, no idRef)   |
///
/// Unknown getter types conservatively return `true` (allow activation).
fn eval_getter(
    id: i32,
    getters: &[serde_json::Value],
    props: Option<&std::collections::HashMap<String, DynamicParameter>>,
) -> bool {
    let Some(getter) = getters
        .iter()
        .find(|g| g.get("_ID").and_then(|v| v.as_i64()) == Some(id as i64))
    else {
        return true;
    };

    let getter_type = short_trigger_name(
        getter
            .get("$type")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
    );

    match getter_type {
        "BoolGetter" => {
            let path = getter
                .get("_value")
                .and_then(|v| v.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(key) = path.strip_prefix("self_bb.") {
                props
                    .and_then(|p| p.get(key))
                    .and_then(|p| p.value_bool_list.first().copied())
                    .unwrap_or(false)
            } else {
                true
            }
        }
        "BoolGetterInvert" => {
            // Explicit idRef takes precedence; fall back to ID-1 for the
            // paramSource=-1 ("previous result") implicit-chain pattern.
            let inner_id = getter
                .get("_value")
                .and_then(|v| v.get("idRef"))
                .and_then(|v| v.as_i64())
                .unwrap_or(id as i64 - 1);
            !eval_getter(inner_id as i32, getters, props)
        }
        _ => true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_script_state_default_is_enabled() {
        assert_eq!(LevelScriptState::default(), LevelScriptState::Enabled);
    }

    #[test]
    fn level_script_state_repr_values() {
        assert_eq!(LevelScriptState::None as i32, 0);
        assert_eq!(LevelScriptState::Disabled as i32, 1);
        assert_eq!(LevelScriptState::Enabled as i32, 2);
        assert_eq!(LevelScriptState::Active as i32, 3);
    }

    #[test]
    fn short_trigger_name_simple() {
        assert_eq!(short_trigger_name("OnScriptStart"), "OnScriptStart");
    }

    #[test]
    fn short_trigger_name_with_namespace() {
        assert_eq!(
            short_trigger_name("Beyond.LevelScripts.OnScriptStart"),
            "OnScriptStart"
        );
    }

    #[test]
    fn short_trigger_name_with_comma() {
        assert_eq!(
            short_trigger_name(
                "Beyond.LevelScripts.OnScriptStart,Beyond.LevelScripts.OnScriptActive"
            ),
            "OnScriptStart"
        );
    }

    #[test]
    fn short_trigger_name_empty_string() {
        assert_eq!(short_trigger_name(""), "");
    }

    #[test]
    fn shape_contains_point_inside() {
        let shape = config::tables::level_data::LvShape {
            offset: config::tables::level_data::Vector3f {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            radius: 10.0,
            ..Default::default()
        };
        assert!(shape_contains_point(&shape, (5.0, 0.0, 0.0)));
    }

    #[test]
    fn shape_contains_point_on_boundary() {
        let shape = config::tables::level_data::LvShape {
            offset: config::tables::level_data::Vector3f {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            radius: 10.0,
            ..Default::default()
        };
        // Exactly at radius distance
        assert!(shape_contains_point(&shape, (10.0, 0.0, 0.0)));
    }

    #[test]
    fn shape_contains_point_outside() {
        let shape = config::tables::level_data::LvShape {
            offset: config::tables::level_data::Vector3f {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            radius: 10.0,
            ..Default::default()
        };
        assert!(!shape_contains_point(&shape, (15.0, 0.0, 0.0)));
    }

    #[test]
    fn shape_contains_point_with_offset() {
        let shape = config::tables::level_data::LvShape {
            offset: config::tables::level_data::Vector3f {
                x: 100.0,
                y: 0.0,
                z: 100.0,
            },
            radius: 10.0,
            ..Default::default()
        };
        assert!(shape_contains_point(&shape, (105.0, 0.0, 100.0)));
        assert!(!shape_contains_point(&shape, (0.0, 0.0, 0.0)));
    }

    #[test]
    fn script_initial_state_known_overrides() {
        let script = config::tables::level_data::LvLevelScript {
            script_id: 70001,
            ..Default::default()
        };
        assert_eq!(
            script_initial_state(&script, "map01_lv001"),
            LevelScriptState::Active
        );
    }

    #[test]
    fn script_initial_state_default_is_enabled() {
        let script = config::tables::level_data::LvLevelScript {
            script_id: 99999,
            ..Default::default()
        };
        assert_eq!(
            script_initial_state(&script, "unknown_scene"),
            LevelScriptState::Enabled
        );
    }

    #[test]
    fn has_root_on_script_start_true() {
        let headers = vec![serde_json::json!({
            "$type": "OnScriptStart",
            "_ID": 0
        })];
        assert!(has_root_on_script_start(&headers));
    }

    #[test]
    fn has_root_on_script_start_false_non_root() {
        let headers = vec![serde_json::json!({
            "$type": "OnScriptStart",
            "_ID": 5
        })];
        assert!(!has_root_on_script_start(&headers));
    }

    #[test]
    fn has_root_on_script_start_false_wrong_type() {
        let headers = vec![serde_json::json!({
            "$type": "OnScriptActive",
            "_ID": 0
        })];
        assert!(!has_root_on_script_start(&headers));
    }

    #[test]
    fn has_root_on_script_start_empty() {
        let headers: Vec<serde_json::Value> = vec![];
        assert!(!has_root_on_script_start(&headers));
    }

    #[test]
    fn level_script_manager_default() {
        let mgr = LevelScriptManager::default();
        assert!(mgr.current_scene.is_empty());
    }

    #[test]
    fn try_consume_progression_flag_first_call_true() {
        let mut mgr = LevelScriptManager::default();
        assert!(mgr.try_consume_progression_flag("test_scene", 1, "flag_a"));
    }

    #[test]
    fn try_consume_progression_flag_duplicate_false() {
        let mut mgr = LevelScriptManager::default();
        mgr.try_consume_progression_flag("test_scene", 1, "flag_a");
        assert!(!mgr.try_consume_progression_flag("test_scene", 1, "flag_a"));
    }

    #[test]
    fn try_consume_progression_flag_different_flag_true() {
        let mut mgr = LevelScriptManager::default();
        mgr.try_consume_progression_flag("test_scene", 1, "flag_a");
        assert!(mgr.try_consume_progression_flag("test_scene", 1, "flag_b"));
    }

    #[test]
    fn try_consume_server_event_first_call_true() {
        let mut mgr = LevelScriptManager::default();
        assert!(mgr.try_consume_server_event("test_scene", 1, "event_a"));
    }

    #[test]
    fn try_consume_server_event_duplicate_false() {
        let mut mgr = LevelScriptManager::default();
        mgr.try_consume_server_event("test_scene", 1, "event_a");
        assert!(!mgr.try_consume_server_event("test_scene", 1, "event_a"));
    }
}
