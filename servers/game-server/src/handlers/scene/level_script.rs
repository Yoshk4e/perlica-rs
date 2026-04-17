//! Level-script state updates and event triggers.

use crate::net::NetContext;
use perlica_proto::{
    CsSceneCommitLevelScriptCacheStep, CsSceneLevelScriptEventTrigger, CsSceneSetLevelScriptActive,
    CsSceneUpdateInteractiveProperty, CsSceneUpdateLevelScriptProperty,
    ScSceneLevelScriptEventTrigger, ScSceneUpdateInteractiveProperty,
    ScSceneUpdateLevelScriptProperty,
};
use tracing::debug;

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
