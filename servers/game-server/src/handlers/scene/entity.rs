//! Entity lifecycle: create, destroy, and dynamic monster spawning.

use crate::net::NetContext;
use perlica_logic::scene::EntityDestroyReason;
use perlica_logic::traits::Classified;
use perlica_proto::{
    CsSceneCreateEntity, CsSceneDestroyEntity, ScSceneCreateEntity, SceneMonster,
    SceneObjectCommonInfo, Vector,
};
use tracing::{debug, error, warn};

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
/// The client may suggest an ID in `info.id`, but the server shouldn't
/// trust it. If the requested ID is already in use, is zero, or falls
/// in the character-object-ID range (< 1000), the server allocates a
/// fresh ID from `EntityManager::next_monster_id()`.
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

    for info in &req.entity_infos {
        // Character object IDs are 1-based and typically small,
        // monster IDs start at 1000. We never allow the client to pick
        // an ID that collides with an existing entity or falls in the
        // character-ID range.
        let needs_server_id = info.id == 0
            || info.id < 1000 // character-ID range, reserved
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

    for id in req.id_list {
        if let Some(removed) = ctx.player.entities.remove(id) {
            // Only enemies record a respawn cooldown.  Interactives /
            // NPCs just need their interest entry cleared.
            if removed.is_enemy() {
                ctx.player.scene.on_entity_killed(removed.level_logic_id);
            } else {
                // Non-enemy: skip the dead_entities cooldown but still
                // flush the interest entry so the cap stays accurate.
                ctx.player.scene.on_entity_despawned(removed.level_logic_id);
            }
        }

        let msg = ctx
            .player
            .scene
            .destroy_entity(id, EntityDestroyReason::Dead);

        if let Err(error) = ctx.notify(msg).await {
            error!(
                "Failed to send entity destroy notification for {}: {:?}",
                id, error
            );
        }
    }
}
