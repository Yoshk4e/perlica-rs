use crate::net::NetContext;
use perlica_logic::character::char_bag::CharIndex;
use perlica_logic::scene::EntityDestroyReason;
use perlica_proto::{
    BattleInfo, CsFinishDialog, CsSceneCommitLevelScriptCacheStep, CsSceneCreateEntity,
    CsSceneDestroyEntity, CsSceneKillChar, CsSceneKillMonster, CsSceneLevelScriptEventTrigger,
    CsSceneLoadFinish, CsSceneRevival, CsSceneSetLastRecordCampid, CsSceneSetLevelScriptActive,
    CsSceneTeleport, CsSceneUpdateInteractiveProperty, CsSceneUpdateLevelScriptProperty,
    ScCharSyncStatus, ScEnterSceneNotify, ScFinishDialog, ScObjectEnterView, ScSceneCreateEntity,
    ScSceneLevelScriptEventTrigger, ScSceneSetLastRecordCampid, ScSceneTeleport,
    ScSceneUpdateInteractiveProperty, ScSceneUpdateLevelScriptProperty, ScSelfSceneInfo, Vector,
};
use tracing::{debug, error, info};

/// Pushes `ScEnterSceneNotify` during login before the client has finished loading.
pub async fn notify_enter_scene(ctx: &mut NetContext<'_>) -> bool {
    let msg = ScEnterSceneNotify {
        role_id: 1,
        scene_name: ctx.player.world.last_scene.clone(),
        scene_id: ctx
            .assets
            .str_id_num
            .get_scene_id(&ctx.player.world.last_scene)
            .unwrap_or(0),
        position: Some(Vector {
            x: ctx.player.movement.pos_x,
            y: ctx.player.movement.pos_y,
            z: ctx.player.movement.pos_z,
        }),
    };

    debug!("Entering scene: {}", msg.scene_name);

    if let Err(error) = ctx.notify(msg).await {
        error!("Failed to send enter scene notification: {:?}", error);
        return false;
    }

    true
}

/// Handles `CsSceneLoadFinish`. Finalises scene state and syncs all entities and character state.
pub async fn on_scene_load_finish(
    ctx: &mut NetContext<'_>,
    req: CsSceneLoadFinish,
) -> ScSelfSceneInfo {
    info!("Scene load finished: {}", req.scene_name);

    ctx.player.world.last_scene = req.scene_name.clone();

    let (enter_view, self_info) = ctx.player.scene.finish_scene_load(
        &ctx.player.char_bag,
        &ctx.player.movement,
        ctx.assets,
        &mut ctx.player.entities,
    );

    if let Err(error) = ctx.notify(enter_view).await {
        error!("Failed to send object enter view: {:?}", error);
    }

    let pos = ctx.player.movement.position_tuple();
    let (initial_enter, _) =
        ctx.player
            .scene
            .update_visible_entities(pos, ctx.assets, &mut ctx.player.entities);

    if let Some(msg) = initial_enter {
        let _ = ctx.notify(msg).await.inspect_err(|error| {
            error!("Failed to send initial dynamic enter view: {:?}", error);
        });
    }

    if !crate::handlers::factory::push_factory(ctx).await {
        error!("Failed to sync factory context");
    }

    if !post_load_sync(ctx).await {
        error!("Failed to complete post-load sync");
    }

    self_info
}

async fn post_load_sync(ctx: &mut NetContext<'_>) -> bool {
    let ok_attrs = super::char_bag::push_char_attrs(ctx).await;
    let ok_status = super::char_bag::push_char_status(ctx).await;
    ok_attrs && ok_status
}

/// Removes a monster entity and notifies the client with `ScSceneDestroyEntity`.
pub async fn on_cs_scene_kill_monster(ctx: &mut NetContext<'_>, req: CsSceneKillMonster) {
    debug!("Monster killed: {}", req.id);

    if let Some(entity) = ctx
        .player
        .entities
        .remove(req.id)
        .filter(|e| e.kind == perlica_logic::entity::EntityKind::Enemy)
    {
        ctx.player
            .scene
            .dead_entities
            .insert(entity.level_logic_id, common::time::now_ms());
    }

    let msg = ctx
        .player
        .scene
        .destroy_entity(req.id, EntityDestroyReason::Dead);

    if let Err(error) = ctx.notify(msg).await {
        error!("Failed to send monster kill notification: {:?}", error);
    }
}

/// Marks a character dead and destroys the scene entity. The `CharBag` entry is preserved for revival.
pub async fn on_cs_scene_kill_char(ctx: &mut NetContext<'_>, req: CsSceneKillChar) {
    debug!("Character killed: {}", req.id);

    if let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.id) {
        char_data.is_dead = true;
    }

    let msg = ctx
        .player
        .scene
        .destroy_entity(req.id, EntityDestroyReason::Dead);

    if let Err(error) = ctx.notify(msg).await {
        error!("Failed to send character kill notification: {:?}", error);
    }
}

/// Handles `CsSceneRevival`, revives all dead characters in the current team
/// at 50 % HP.
/// ORDER MATTERS!!
/// Send order:
/// 1. `ScCharSyncStatus` × N, HP per revived char
/// 2. `ScSelfSceneInfo` with `revive_chars`, triggers client revival logic
/// 3. `ScSceneRevival` — revival UI/effect
/// 4. `ScObjectEnterView` — reply, re-enters chars into the scene
pub async fn on_cs_scene_revival(
    ctx: &mut NetContext<'_>,
    _req: CsSceneRevival,
) -> ScObjectEnterView {
    info!("Scene revival requested");

    let (enter_view, self_info, revival) = ctx.player.scene.handle_revival(
        &mut ctx.player.char_bag,
        &ctx.player.movement,
        ctx.assets,
        &mut ctx.player.entities,
        None,
    );

    send_revival_status_updates(ctx).await;

    if let Err(error) = ctx.notify(self_info).await {
        error!("Failed to send revival self info: {:?}", error);
    }

    if let Err(error) = ctx.notify(revival).await {
        error!("Failed to send revival notification: {:?}", error);
    }

    enter_view
}

async fn send_revival_status_updates(ctx: &mut NetContext<'_>) {
    let team_idx = ctx.player.char_bag.meta.curr_team_index as usize;
    let team = &ctx.player.char_bag.teams[team_idx];

    let updates: Vec<(u64, f64, f32)> = ctx
        .player
        .char_bag
        .chars
        .iter()
        .enumerate()
        .filter(|(i, c)| {
            !c.is_dead
                && team.char_team.iter().any(|slot| {
                    slot.char_index()
                        .map(|idx| idx.as_usize() == *i)
                        .unwrap_or(false)
                })
        })
        .map(|(i, c)| (CharIndex::from_usize(i).object_id(), c.hp, c.ultimate_sp))
        .collect();

    for (objid, hp, ultimatesp) in updates {
        if let Err(error) = ctx
            .notify(ScCharSyncStatus {
                objid,
                is_dead: false,
                battle_info: Some(BattleInfo { hp, ultimatesp }),
            })
            .await
        {
            error!(
                "Failed to send revival status update for {}: {:?}",
                objid, error
            );
        }
    }
}

/// Spawns a monster and returns the create/enter-view pair.
/// `ScSceneCreateEntity` carries only the ID; full detail goes in `ScObjectEnterView`.
pub fn spawn_dynamic_monster(
    ctx: &mut NetContext<'_>,
    template_id: String,
    position: Vector,
    level: i32,
    entity_type: i32,
    level_logic_id: u64,
) -> (
    perlica_proto::ScSceneCreateEntity,
    perlica_proto::SceneMonster,
) {
    use perlica_logic::entity::{EntityKind, SceneEntity};

    let id = ctx.player.entities.next_monster_id();

    ctx.player.entities.insert(SceneEntity {
        id,
        template_id: template_id.clone(),
        kind: EntityKind::Enemy,
        pos_x: position.x,
        pos_y: position.y,
        pos_z: position.z,
        level_logic_id,
        belong_level_script_id: 0,
    });

    let create = ctx.player.scene.create_entity(id);

    let monster = perlica_proto::SceneMonster {
        common_info: Some(perlica_proto::SceneObjectCommonInfo {
            id,
            r#type: entity_type,
            templateid: template_id,
            position: Some(position),
            rotation: None,
            belong_level_script_id: 0,
        }),
        origin_id: level_logic_id,
        level,
    };

    (create, monster)
}

pub fn entity_exists(ctx: &NetContext<'_>, entity_id: u64) -> bool {
    ctx.player.entities.contains(entity_id)
}

pub fn current_scene_name<'a>(ctx: &'a NetContext<'_>) -> &'a str {
    ctx.player.scene.scene_name()
}

/*pub async fn on_cs_scene_interactive_event_trigger(
    ctx: &mut NetContext<'_>,
    req: CsSceneInteractiveEventTrigger,
) -> ScSceneInteractiveEventTrigger {
    debug!(
        "Interactive event trigger: scene={}, id={}, event={}, props={:?}",
        req.scene_name, req.id, req.event_name, req.properties
    );

    let level_logic_id = perlica_logic::scene::level_logic_id_from_interactive(req.id);

    let is_campfire = ctx
        .assets
        .level_data
        .interactives(&req.scene_name)
        .iter()
        .find(|i| i.base.level_logic_id == level_logic_id)
        .map(|i| i.base.template_id.as_str() == "int_campfire")
        .unwrap_or(false);

    if is_campfire && req.event_name == "activate" {
        info!(
            "Activating campfire: scene={}, id={}, level_logic_id={}",
            req.scene_name, req.id, level_logic_id
        );

        let mut properties = std::collections::HashMap::new();
        properties.insert(
            "is_on".to_string(),
            DynamicParameter {
                value_type: 3,
                real_type: 3,
                value_int_list: vec![1],
                ..Default::default()
            },
        );

        let update = ScSceneUpdateInteractiveProperty {
            scene_name: req.scene_name.clone(),
            id: req.id,
            properties,
            client_operate: false,
        };

        println!("update: {:?}", update);

        let msg = ScSceneSetSafeZone {
            id: req.id,
            in_zone: true,
        };

        ctx.notify(update).await;
        if let Err(error) = ctx.notify(msg).await {
            error!(
                "Failed to send campfire safe-zone packet for {}: {:?}",
                req.id, error
            );
        }
    }

    ScSceneInteractiveEventTrigger {}
}*/
//this was commented out because it really doesn't work

/// Stores the campfire as the current checkpoint so revival/repatriation return here.
///
/// Send order:
///   1. `ScSceneSetLastRecordCampid` — ACK echoing the camp id back.
pub async fn on_cs_scene_set_last_record_campid(
    ctx: &mut NetContext<'_>,
    req: CsSceneSetLastRecordCampid,
) -> ScSceneSetLastRecordCampid {
    info!("Setting last campfire: camp_id={}", req.last_camp_id);

    if let Some(pos) = &req.position {
        let checkpoint = perlica_logic::scene::CheckpointInfo {
            scene_name: ctx.player.scene.scene_name().to_string(),
            pos_x: pos.x,
            pos_y: pos.y,
            pos_z: pos.z,
        };
        ctx.player.scene.set_checkpoint(checkpoint);
    }

    ctx.player
        .scene
        .set_revival_mode(perlica_logic::scene::RevivalMode::CheckPoint);

    ScSceneSetLastRecordCampid {
        last_camp_id: req.last_camp_id,
    }
}

/// Registers client-spawned entities server-side and echoes back `ScSceneCreateEntity`.
pub async fn on_cs_scene_create_entity(
    ctx: &mut NetContext<'_>,
    req: CsSceneCreateEntity,
) -> ScSceneCreateEntity {
    debug!(
        "Scene create entity: scene={}, entities={:?}",
        req.scene_name, req.entity_infos
    );

    for info in &req.entity_infos {
        if info.id != 0 && !ctx.player.entities.contains(info.id) {
            ctx.player
                .entities
                .insert(perlica_logic::entity::SceneEntity {
                    id: info.id,
                    template_id: String::new(),
                    kind: perlica_logic::entity::EntityKind::Creature,
                    pos_x: 0.0,
                    pos_y: 0.0,
                    pos_z: 0.0,
                    level_logic_id: 0,
                    belong_level_script_id: 0,
                });
        }
    }

    let echo_id = req.entity_infos.first().map(|e| e.id).unwrap_or(0);

    ctx.player.scene.create_entity(echo_id)
}

/// Removes entities reported destroyed by the client.
pub async fn on_cs_scene_destroy_entity(ctx: &mut NetContext<'_>, req: CsSceneDestroyEntity) {
    debug!(
        "Scene destroy entities: scene={}, ids={:?}, reason={}",
        req.scene_name, req.id_list, req.reason
    );

    for id in req.id_list {
        ctx.player.entities.remove(id);

        let msg = ctx
            .player
            .scene
            .destroy_entity(id, EntityDestroyReason::Immediately);

        if let Err(error) = ctx.notify(msg).await {
            error!(
                "Failed to send entity destroy notification for {}: {:?}",
                id, error
            );
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

pub async fn on_cs_finish_dialog(ctx: &mut NetContext<'_>, req: CsFinishDialog) -> ScFinishDialog {
    info!(
        "Dialog finished: dialog_id={}, trunks={:?}",
        req.dialog_id, req.trunk_id_list
    );

    let scene_name = ctx.player.scene.scene_name().to_string();
    let activated = ctx
        .player
        .scene
        .level_scripts
        .on_dialog_finished(&scene_name, ctx.assets);
    for script_id in activated {
        if let Some(notify) = ctx
            .player
            .scene
            .level_scripts
            .state_notify(&scene_name, script_id)
        {
            let _ = ctx.notify(notify).await;
        }
    }

    ScFinishDialog {
        dialog_id: req.dialog_id,
        trunk_id_list: req.trunk_id_list,
    }
}

pub async fn on_cs_scene_teleport(
    ctx: &mut NetContext<'_>,
    req: CsSceneTeleport,
) -> ScSceneTeleport {
    debug!(
        "Scene teleport: scene={}, position={:?}, rotation={:?}, reason={}",
        req.scene_name, req.position, req.rotation, req.teleport_reason
    );

    // Only wipe and re-initialise level-script state when we are actually
    // moving to a *different* scene. Intra-scene warps (reason=1, same
    // scene name) must preserve all existing script runtime.
    let is_scene_change = ctx.player.scene.current_scene != req.scene_name;

    ctx.player.world.last_scene = req.scene_name.clone();
    ctx.player.scene.current_scene = req.scene_name.clone();
    if is_scene_change {
        ctx.player.entities.clear();
        ctx.player.scene.dead_entities.clear();
        ctx.player
            .scene
            .level_scripts
            .reset_scene(&req.scene_name, ctx.assets);
    } else {
        // Same scene: ensure any scripts that have not been initialised yet
        // get their initial state, but leave all existing runtime intact.
        ctx.player
            .scene
            .level_scripts
            .sync_scene(&req.scene_name, ctx.assets);
    }
    ctx.player.scene.scene_id = ctx
        .assets
        .str_id_num
        .get_scene_id(&req.scene_name)
        .unwrap_or(ctx.player.scene.scene_id);

    let position = req.position.unwrap_or(perlica_proto::Vector {
        x: ctx.player.movement.pos_x,
        y: ctx.player.movement.pos_y,
        z: ctx.player.movement.pos_z,
    });
    ctx.player
        .movement
        .update_position(position.x, position.y, position.z);
    let rotation_vec = req.rotation.unwrap_or(perlica_proto::Vector {
        x: ctx.player.movement.rot_x,
        y: ctx.player.movement.rot_y,
        z: ctx.player.movement.rot_z,
    });
    ctx.player
        .movement
        .update_rotation(rotation_vec.x, rotation_vec.y, rotation_vec.z);
    ctx.player.movement.sync_to_world(&mut ctx.player.world);

    let team_idx = ctx.player.char_bag.meta.curr_team_index as usize;
    let obj_id_list = ctx
        .player
        .char_bag
        .teams
        .get(team_idx)
        .map(|team| {
            team.char_team
                .iter()
                .filter_map(|slot| slot.object_id())
                .collect::<Vec<u64>>()
        })
        .unwrap_or_default();

    ctx.player.scene.teleport(
        obj_id_list,
        position,
        Some(rotation_vec),
        common::time::now_ms() as u32,
        req.teleport_reason,
        None,
    )
}
