//! Entity lifecycle: create, destroy, and dynamic monster spawning.

use crate::net::NetContext;
use perlica_logic::scene::EntityDestroyReason;
use perlica_proto::{
    CsSceneCreateEntity, CsSceneDestroyEntity, ScSceneCreateEntity, SceneMonster,
    SceneObjectCommonInfo, Vector,
};
use tracing::{debug, error};

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
            .destroy_entity(id, EntityDestroyReason::Dead);

        if let Err(error) = ctx.notify(msg).await {
            error!(
                "Failed to send entity destroy notification for {}: {:?}",
                id, error
            );
        }
    }
}
