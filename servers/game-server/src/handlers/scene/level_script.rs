//! Level-script state updates and event triggers.

use crate::net::NetContext;
use perlica_proto::{
    CsSceneCommitLevelScriptCacheStep, CsSceneLevelScriptEventTrigger, CsSceneSetLevelScriptActive,
    CsSceneUpdateInteractiveProperty, CsSceneUpdateLevelScriptProperty, DynamicParameter,
    MissionState, QuestState, RoleBaseInfo, ScSceneLevelScriptEventTrigger,
    ScSceneUpdateInteractiveProperty, ScSceneUpdateLevelScriptProperty,
};
use tracing::debug;

/// Server-driven prologue / tutorial flags. When the client flips ANY of
/// these to `true` **for the first time in this session**, the server pushes
/// one objective step for the tracked quest.
///
/// Rules for adding flags here:
///  - The flag must represent a distinct, player-caused event.
///  - Do NOT add a flag that is set by the same action chain as another flag
///    already listed (it would fire a second packet for the same event and
///    double-advance the quest).
///
/// Known exclusions:
///  - `is_G_Ob_Over` (script 5): set by action 29 immediately after
///    `isWalkLimitFinish` (action 28) in the walk-limit leave-trigger chain.
///    One player event -> two property packets -> two advances.  Only
///    `isWalkLimitFinish` belongs here.
fn is_progression_flag(scene: &str, script_id: i32, key: &str) -> bool {
    matches!(
        (scene, script_id, key),
        // map01_dg003 prologue tutorial controller (script 5)
        ("map01_dg003", 5, "isFMVOver")
        | ("map01_dg003", 5, "isTimelineOver")
        | ("map01_dg003", 5, "isWalkLimitFinish")
        | ("map01_dg003", 5, "isRunLimitFinish")
        // map01_dg003 vista / interaction sub-steps
        | ("map01_dg003", 10, "isPreBattleOver")
        | ("map01_dg003", 14, "is_spot_interacted")
        | ("map01_dg003", 15, "isBombVistaOver")
        | ("map01_dg003", 17, "isTreasureVistaOver")
        | ("map01_dg003", 17, "is_G_Jump_Over")
        | ("map01_dg003", 18, "isPatriotVistaOver")
        | ("map01_dg003", 19, "is_barrierwal_interacted")
        | ("map01_dg003", 19, "isFogChangeOver")
        | ("map01_dg003", 19, "isCemeteryVistaOver")
        | ("map01_dg003", 21, "isTowerVistaOver")
    )
}

fn property_is_true(p: &DynamicParameter) -> bool {
    p.value_bool_list.first().copied().unwrap_or(false)
        || p.value_int_list.first().copied().unwrap_or(0) != 0
}

fn build_role_base_info(ctx: &NetContext<'_>) -> RoleBaseInfo {
    RoleBaseInfo {
        leader_char_id: ctx.player.get_leader_objid(),
        leader_position: Some(perlica_proto::Vector {
            x: *ctx.player.movement.pos.get_x(),
            y: *ctx.player.movement.pos.get_y(),
            z: *ctx.player.movement.pos.get_z(),
        }),
        leader_rotation: Some(perlica_proto::Vector {
            x: *ctx.player.movement.rot.get_x(),
            y: *ctx.player.movement.rot.get_y(),
            z: *ctx.player.movement.rot.get_z(),
        }),
        scene_name: ctx.player.scene.scene_name().to_string(),
        server_ts: common::time::now_ms(),
    }
}

/// Walk every property in this packet; for each flag that:
///   1. is in the `is_progression_flag` allowlist,
///   2. is being flipped to `true`, AND
///   3. has NOT already been consumed this session (idempotency guard),
/// push one objective step to the tracked quest.
///
/// Notification order: state_updates BEFORE notify_objective_updates.
/// The client HUD resolves quest tracking data by quest-ID the moment an
/// objective-update packet arrives.  Sending state first ensures the client
/// already knows the quest is Qsprocessing, so trackQuestData is not nil.
async fn maybe_drive_quest_progression<'a>(
    ctx: &mut NetContext<'a>,
    scene: &str,
    script_id: i32,
    properties: &std::collections::HashMap<String, DynamicParameter>,
) {
    // Collect flags that are (a) known progressors, (b) newly true, and
    // (c) not yet consumed this session.  Advancing once per consumed flag
    // lets two flags in the same packet each advance their own step without
    // collapsing into a single advance or double-advancing the same step.
    let mut advance_count = 0u32;
    for (key, value) in properties {
        if !is_progression_flag(scene, script_id, key) || !property_is_true(value) {
            continue;
        }
        if ctx
            .player
            .scene
            .level_scripts
            .try_consume_progression_flag(scene, script_id, key)
        {
            advance_count += 1;
            debug!(
                "Server-driven quest progression triggered by {}::{}::{}",
                scene, script_id, key
            );
        } else {
            debug!(
                "Skipping already-consumed progression flag {}::{}::{}",
                scene, script_id, key
            );
        }
    }
    if advance_count == 0 {
        return;
    }

    let role_base_info = Some(build_role_base_info(ctx));

    for _ in 0..advance_count {
        let update = ctx
            .player
            .missions
            .advance_tracked_quest_step(&ctx.assets.missions, role_base_info.clone());

        // 1. Updated objectives for the quest whose step was just completed.
        if let Some(reply) = update.reply_objective_update {
            let _ = ctx.notify(reply).await;
        }

        // 2. State changes first: client must know a quest is Qsprocessing
        //    before we send objective packets for that quest id.
        for s in &update.state_updates {
            let _ = ctx.notify(s.clone()).await;
            if let Ok(qs) = QuestState::try_from(s.quest_state) {
                let scene_name = ctx.player.scene.scene_name().to_string();
                let activated = ctx.player.scene.level_scripts.on_quest_state_changed(
                    &scene_name,
                    &s.quest_id,
                    qs,
                    ctx.assets,
                );
                for sid in activated {
                    if let Some(n) = ctx
                        .player
                        .scene
                        .level_scripts
                        .state_notify(&scene_name, sid)
                    {
                        let _ = ctx.notify(n).await;
                    }
                }
            }
        }
        for m in &update.mission_updates {
            let _ = ctx.notify(m.clone()).await;
            if let Ok(ms) = MissionState::try_from(m.mission_state) {
                let scene_name = ctx.player.scene.scene_name().to_string();
                let activated = ctx.player.scene.level_scripts.on_mission_state_changed(
                    &scene_name,
                    &m.mission_id,
                    ms,
                    ctx.assets,
                );
                for sid in activated {
                    if let Some(n) = ctx
                        .player
                        .scene
                        .level_scripts
                        .state_notify(&scene_name, sid)
                    {
                        let _ = ctx.notify(n).await;
                    }
                }
            }
        }

        // 3. New quest's objectives last, client already received the
        //    ScQuestStateUpdate so trackQuestData will resolve correctly.
        for n in update.notify_objective_updates {
            let _ = ctx.notify(n).await;
        }
    }
}

pub async fn on_cs_scene_set_level_script_active(
    ctx: &mut NetContext<'_>,
    req: CsSceneSetLevelScriptActive,
) {
    debug!(
        "Set level script active: scene={}, script_id={}, is_active={}",
        req.scene_name, req.script_id, req.is_active
    );

    let level_scripts = &mut ctx.player.scene.level_scripts;

    if let Some(notify) = level_scripts
        .set_client_active(&req.scene_name, req.script_id, req.is_active, ctx.assets)
        .and_then(|_| level_scripts.state_notify(&req.scene_name, req.script_id))
    {
        let _ = ctx.notify(notify).await;
    }
}

/// Updates level script properties and echoes them back with `client_operate = true`.
/// (per disassembly, confirms the client as originator)
pub async fn on_cs_scene_update_level_script_property(
    ctx: &mut NetContext<'_>,
    req: CsSceneUpdateLevelScriptProperty,
) -> ScSceneUpdateLevelScriptProperty {
    debug!(
        "Update level script property: scene={}, script_id={}, props={:?}",
        req.scene_name, req.script_id, req.properties
    );

    ctx.player.scene.level_scripts.update_properties(
        &req.scene_name,
        req.script_id,
        &req.properties,
        ctx.assets,
    );

    maybe_drive_quest_progression(ctx, &req.scene_name, req.script_id, &req.properties).await;

    ScSceneUpdateLevelScriptProperty {
        scene_name: req.scene_name,
        script_id: req.script_id,
        properties: req.properties,
        client_operate: true,
    }
}

pub async fn on_cs_scene_update_interactive_property(
    _ctx: &mut NetContext<'_>,
    req: CsSceneUpdateInteractiveProperty,
) -> ScSceneUpdateInteractiveProperty {
    debug!(
        "Update interactive property: scene={}, id={}, props={:?}",
        req.scene_name, req.id, req.properties
    );

    ScSceneUpdateInteractiveProperty {
        scene_name: req.scene_name,
        id: req.id,
        properties: req.properties,
        client_operate: true,
    }
}

pub async fn on_cs_scene_level_script_event_trigger(
    ctx: &mut NetContext<'_>,
    req: CsSceneLevelScriptEventTrigger,
) -> ScSceneLevelScriptEventTrigger {
    debug!(
        "Level script event trigger: scene={}, script_id={}, event={}, props={:?}",
        req.scene_name, req.script_id, req.event_name, req.properties
    );

    ctx.player.scene.level_scripts.update_properties(
        &req.scene_name,
        req.script_id,
        &req.properties,
        ctx.assets,
    );

    let activated = ctx.player.scene.level_scripts.on_custom_event(
        &req.scene_name,
        &req.event_name,
        ctx.assets,
    );
    for script_id in activated {
        if let Some(notify) = ctx
            .player
            .scene
            .level_scripts
            .state_notify(&req.scene_name, script_id)
        {
            let _ = ctx.notify(notify).await;
        }
    }

    // 2. Catch spatial scripts the client missed (e.g. player was on a
    //    spline and didn't send a SetLevelScriptActive packet).
    //    This picks up script 20001 (LeaveSplineMove) when the player is
    //    within its activeShape but not its startShape.
    let player_pos = (
        *ctx.player.movement.pos.get_x(),
        *ctx.player.movement.pos.get_y(),
        *ctx.player.movement.pos.get_z(),
    );
    let proximate = ctx
        .player
        .scene
        .level_scripts
        .activate_eligible_proximate_scripts(&req.scene_name, player_pos, ctx.assets);
    for script_id in proximate {
        if let Some(notify) = ctx
            .player
            .scene
            .level_scripts
            .state_notify(&req.scene_name, script_id)
        {
            let _ = ctx.notify(notify).await;
        }
    }

    // 3. Catch shape-less OnScriptActive scripts designed for server
    //    activation (e.g. script 20006 - end-game FMV/cutscene sequence).
    //    The client can never trigger these by proximity because they
    //    have no spatial shapes.
    let server_triggered = ctx
        .player
        .scene
        .level_scripts
        .activate_server_triggered_scripts(&req.scene_name, ctx.assets);
    for script_id in server_triggered {
        if let Some(notify) = ctx
            .player
            .scene
            .level_scripts
            .state_notify(&req.scene_name, script_id)
        {
            let _ = ctx.notify(notify).await;
        }
    }

    maybe_drive_quest_progression(ctx, &req.scene_name, req.script_id, &req.properties).await;

    ScSceneLevelScriptEventTrigger {}
}

pub async fn on_cs_scene_commit_level_script_cache_step(
    ctx: &mut NetContext<'_>,
    req: CsSceneCommitLevelScriptCacheStep,
) {
    debug!(
        "Commit level script cache step: scene={}, script_id={}",
        req.scene_name, req.script_id
    );

    let level_scripts = &mut ctx.player.scene.level_scripts;

    if let Some(notify) = level_scripts
        .commit_cache_step(&req.scene_name, req.script_id, ctx.assets)
        .and_then(|_| level_scripts.state_notify(&req.scene_name, req.script_id))
    {
        let _ = ctx.notify(notify).await;
    }
}
