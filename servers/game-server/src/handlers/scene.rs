use crate::net::NetContext;
use perlica_logic::character::char_bag::CharIndex;
use perlica_logic::scene::{EntityDestroyReason, SelfInfoReason};
use perlica_proto::{
    BattleInfo, CsSceneInteractiveEventTrigger, CsSceneKillChar, CsSceneKillMonster,
    CsSceneLoadFinish, CsSceneRevival, ScCharSyncStatus, ScEnterSceneNotify, ScObjectEnterView,
    ScSceneInteractiveEventTrigger, ScSelfSceneInfo, Vector,
};
use tracing::{debug, error, info};

/// Sends `ScEnterSceneNotify` to push the player into their last known scene.
///
/// Called during the login sequence before the client has acknowledged loading.
/// Returns `false` if the notification could not be sent.
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

/// Handles `CsSceneLoadFinish` — the client confirming it has finished loading
/// the scene.
///
/// Finalises the server-side scene state, then sends `ScObjectEnterView` for
/// all characters and monsters before returning `ScSelfSceneInfo`. Also pushes
/// character attributes and status so the client is fully in sync from the
/// moment gameplay begins.
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
        if let Err(error) = ctx.notify(msg).await {
            error!("Failed to send initial dynamic enter view: {:?}", error);
        }
    }

    if !crate::handlers::factory::push_factory(ctx).await {
        error!("Failed to sync factory context");
    }

    if !post_load_sync(ctx).await {
        error!("Failed to complete post-load sync");
    }

    self_info
}

/// Pushes character attributes and HP/is_dead status after a scene load.
async fn post_load_sync(ctx: &mut NetContext<'_>) -> bool {
    let ok_attrs = super::char_bag::push_char_attrs(ctx).await;
    let ok_status = super::char_bag::push_char_status(ctx).await;
    ok_attrs && ok_status
}

/// Handles `CsSceneKillMonster` — removes the entity from the manager and
/// notifies the client with `ScSceneDestroyEntity`.
pub async fn on_cs_scene_kill_monster(ctx: &mut NetContext<'_>, req: CsSceneKillMonster) {
    debug!("Monster killed: {}", req.id);

    if let Some(entity) = ctx.player.entities.remove(req.id) {
        if entity.kind == perlica_logic::entity::EntityKind::Enemy {
            ctx.player
                .scene
                .dead_entities
                .insert(entity.level_logic_id, common::time::now_ms());
        }
    }

    let msg = ctx
        .player
        .scene
        .build_entity_destroy(req.id, EntityDestroyReason::Dead);

    if let Err(error) = ctx.notify(msg).await {
        error!("Failed to send monster kill notification: {:?}", error);
    }
}

/// Handles `CsSceneKillChar` — marks the character as dead and notifies the
/// client with `ScSceneDestroyEntity`.
///
/// The character remains in the [`CharBag`] so it can be revived later; only
/// the scene entity is destroyed from the client's perspective.
pub async fn on_cs_scene_kill_char(ctx: &mut NetContext<'_>, req: CsSceneKillChar) {
    debug!("Character killed: {}", req.id);

    if let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.id) {
        char_data.is_dead = true;
    }

    let msg = ctx
        .player
        .scene
        .build_entity_destroy(req.id, EntityDestroyReason::Dead);

    if let Err(error) = ctx.notify(msg).await {
        error!("Failed to send character kill notification: {:?}", error);
    }
}

/// Handles `CsSceneRevival` — revives all dead characters in the current team
/// at 50 % HP.
///
/// Send order:
/// 1. `ScCharSyncStatus` × N — HP per revived char
/// 2. `ScSelfSceneInfo` with `revive_chars` — triggers client revival logic
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

/// Pushes `ScCharSyncStatus` for every alive character in the current team.
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
                objid,
                error
            );
        }
    }
}

/// Dynamically spawns a monster entity and returns the create/enter-view data.
///
/// `ScSceneCreateEntity` carries only `{scene_name, id}` — all entity detail
/// goes in the `SceneMonster` which the caller wraps in `ScObjectEnterView`.
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

    let create = ctx.player.scene.build_entity_create(id);

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

/// Returns `true` if the entity with the given ID is currently tracked in the
/// player's [`EntityManager`].
pub fn entity_exists(ctx: &NetContext<'_>, entity_id: u64) -> bool {
    ctx.player.entities.contains(entity_id)
}

/// Returns the name of the scene the player is currently in.
pub fn current_scene_name<'a>(ctx: &'a NetContext<'_>) -> &'a str {
    ctx.player.scene.scene_name()
}

pub async fn on_cs_scene_interactive_event_trigger(
    _ctx: &mut NetContext<'_>,
    req: CsSceneInteractiveEventTrigger,
) -> ScSceneInteractiveEventTrigger {
    debug!(
        "Interactive event trigger: scene={}, id={}, event={}",
        req.scene_name,
        req.id,
        req.event_name
    );

    ScSceneInteractiveEventTrigger {}
}
