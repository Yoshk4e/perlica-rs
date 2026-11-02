//! Entity lifecycle: create, destroy, and dynamic monster spawning.

use crate::net::NetContext;
use perlica_logic::scene::EntityDestroyReason;
use perlica_logic::traits::Classified;
use perlica_proto::{
    CsSceneCreateEntity, CsSceneDestroyEntity, ScSceneCreateEntity, SceneMonster,
    SceneObjectCommonInfo, Vector,
};
use tracing::{debug, warn};

/// Spawns a monster and returns the create/enter-view pair.
/// `ScSceneCreateEntity` carries only the ID; full detail goes in `ScObjectEnterView`.
pub fn spawn_dynamic_monster(
    ctx: &mut NetContext<'_>,
    template_id: String,
    position: Vector,
    level: i32,
    entity_type: i32,
    level_logic_id: u64,
) -> (ScSceneCreateEntity, SceneMonster) {
    use perlica_logic::entity::{EntityKind, SceneEntity};

    ctx.player
        .scene
        .reserve_level_entity_ids(ctx.assets, &mut ctx.player.entities);
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

    let monster = SceneMonster {
        common_info: Some(SceneObjectCommonInfo {
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

/// Registers client-spawned entities server-side and echoes back `ScSceneCreateEntity`.
///
/// IDs reserved by level data or already in use receive a fresh runtime ID.
pub async fn on_cs_scene_create_entity(
    ctx: &mut NetContext<'_>,
    req: CsSceneCreateEntity,
) -> ScSceneCreateEntity {
    debug!(
        "Scene create entity: scene={}, entities={:?}",
        req.scene_name, req.entity_infos
    );

    // Track the actual IDs assigned so we can echo the first one back.
    let mut assigned_ids: Vec<u64> = Vec::with_capacity(req.entity_infos.len());
    ctx.player
        .scene
        .reserve_level_entity_ids(ctx.assets, &mut ctx.player.entities);

    for info in &req.entity_infos {
        let needs_server_id = info.id == 0
            || info.id < ctx.player.entities.peek_next_monster_id()
            || ctx.player.entities.contains(info.id); // already in use

        let assigned_id = if needs_server_id {
            let new_id = ctx.player.entities.next_monster_id();
            if info.id != 0 {
                warn!(
                    "Rejected client entity id={}, assigned server id={} instead",
                    info.id, new_id
                );
            }
            new_id
        } else {
            info.id
        };

        // there isn't any kind of coords because i still dunno
        ctx.player
            .entities
            .insert(perlica_logic::entity::SceneEntity {
                id: assigned_id,
                template_id: String::new(),
                kind: perlica_logic::entity::EntityKind::Creature,
                pos_x: 0.0,
                pos_y: 0.0,
                pos_z: 0.0,
                level_logic_id: 0,
                belong_level_script_id: 0,
            });

        assigned_ids.push(assigned_id);
    }

    let echo_id = assigned_ids.first().copied().unwrap_or(0);

    ctx.player.scene.create_entity(echo_id)
}

/// Removes entities reported destroyed by the client.
pub async fn on_cs_scene_destroy_entity(ctx: &mut NetContext<'_>, req: CsSceneDestroyEntity) {
    debug!(
        "Scene destroy entities: scene={}, ids={:?}, reason={}",
        req.scene_name, req.id_list, req.reason
    );

    if req.scene_name != ctx.player.scene.current_scene {
        warn!(
            "Ignored entity destruction for a different scene: {}",
            req.scene_name
        );
        return;
    }
    let reason = match req.reason {
        0 => EntityDestroyReason::Immediately,
        1 => EntityDestroyReason::Dead,
        _ => {
            warn!("Ignored unknown entity destruction reason: {}", req.reason);
            return;
        }
    };
    let mut destroyed = std::collections::HashSet::new();
    for id in req.id_list {
        let removed = ctx.player.entities.remove(id);
        if let Some(removed) = &removed {
            if removed.is_enemy() && reason == EntityDestroyReason::Dead {
                ctx.player
                    .scene
                    .on_entity_killed(removed.id, removed.level_logic_id, ctx.now_ms);
            } else {
                ctx.player.scene.on_entity_despawned(removed.id);
            }
        }
        if reason == EntityDestroyReason::Dead
            && removed.as_ref().is_none_or(Classified::is_interactive)
        {
            for linked in ctx.player.scene.destroy_interactive(id, ctx.assets) {
                ctx.player.entities.remove(linked);
                destroyed.insert(linked);
            }
        }
        destroyed.insert(id);
    }
    if let Err(e) = ctx
        .db
        .persist_scene_progress(&ctx.player.uid, &mut ctx.player.scene.progress)
        .await
    {
        warn!(
            "Failed to persist destroyed interactives: uid={}, error={e}",
            ctx.player.uid
        );
    }
    for id in destroyed {
        let msg = ctx.player.scene.destroy_entity(id, reason);
        let _ = ctx.notify(msg).await;
    }
}
