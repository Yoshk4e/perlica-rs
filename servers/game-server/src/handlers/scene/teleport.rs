//! Intra- and inter-scene teleport.
//!
//! Inter-scene teleports wipe entities and reset level-script runtime;
//! intra-scene warps preserve all existing script state so puzzles etc.
//! don't get clobbered.

use crate::net::NetContext;
use perlica_db::Persistable;
use perlica_logic::traits::SyncWriteBack;
use perlica_logic::transform::FiniteVec3;
use perlica_proto::{Code, CsSceneTeleport, ScSceneTeleport, Vector};
use tracing::{debug, warn};

fn current_teleport(ctx: &mut NetContext<'_>, reason: i32) -> ScSceneTeleport {
    let position = Vector {
        x: *ctx.player.movement.pos.get_x(),
        y: *ctx.player.movement.pos.get_y(),
        z: *ctx.player.movement.pos.get_z(),
    };
    let team_idx = ctx.player.char_bag.meta.curr_team_index as usize;
    let obj_id_list = ctx
        .player
        .char_bag
        .teams
        .get(team_idx)
        .map(|team| {
            team.char_team
                .iter()
                .filter_map(perlica_logic::character::char_bag::TeamSlot::object_id)
                .collect::<Vec<u64>>()
        })
        .unwrap_or_default();
    ctx.player.scene.teleport(
        obj_id_list,
        position,
        None,
        common::time::now_ms() as u32,
        reason,
        None,
    )
}

pub async fn on_cs_scene_teleport(
    ctx: &mut NetContext<'_>,
    req: CsSceneTeleport,
) -> ScSceneTeleport {
    debug!(
        "Scene teleport: scene={}, position={:?}, rotation={:?}, reason={}",
        req.scene_name, req.position, req.rotation, req.teleport_reason
    );

    // validate scene name exists in asset tables
    if ctx
        .assets
        .str_id_num
        .get_scene_id(&req.scene_name)
        .is_none()
    {
        warn!(
            "Rejected teleport to unknown scene: '{}' is not in str_id_num",
            req.scene_name
        );
        ctx.send_error(
            Code::ErrSceneNameNotExist,
            format!("unknown scene '{}'", req.scene_name),
        )
        .await;
        return current_teleport(ctx, req.teleport_reason);
    }

    let position = match &req.position {
        Some(v) => FiniteVec3::try_new(v.x, v.y, v.z),
        None => FiniteVec3::try_new(
            *ctx.player.movement.pos.get_x(),
            *ctx.player.movement.pos.get_y(),
            *ctx.player.movement.pos.get_z(),
        ),
    };
    let rotation = match &req.rotation {
        Some(v) => FiniteVec3::try_new(v.x, v.y, v.z),
        None => FiniteVec3::try_new(
            *ctx.player.movement.rot.get_x(),
            *ctx.player.movement.rot.get_y(),
            *ctx.player.movement.rot.get_z(),
        ),
    };
    let (Ok(position), Ok(rotation)) = (position, rotation) else {
        warn!(
            "Rejected teleport with non-finite transform: uid={}, position={:?}, rotation={:?}",
            ctx.player.uid, req.position, req.rotation
        );
        ctx.send_error(
            Code::ErrSceneTeleportPosInvalid,
            "position or rotation contains non-finite value",
        )
        .await;
        return current_teleport(ctx, req.teleport_reason);
    };

    // Only wipe and re-initialise level-script state when we are actually
    // moving to a *different* scene. Intra-scene warps (reason=1, same
    // scene name) must preserve all existing script runtime.
    let is_scene_change = ctx.player.scene.current_scene != req.scene_name;

    ctx.player.world.last_scene.clone_from(&req.scene_name);
    if is_scene_change {
        ctx.player
            .scene
            .reset_for_scene(&req.scene_name, ctx.assets, &mut ctx.player.entities);
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

    ctx.player
        .movement
        .update_position(position.x(), position.y(), position.z());
    ctx.player
        .movement
        .update_rotation(rotation.x(), rotation.y(), rotation.z());
    ctx.player.movement.write_back_into(&mut ctx.player.world);

    let team_idx = ctx.player.char_bag.meta.curr_team_index as usize;
    let obj_id_list = ctx
        .player
        .char_bag
        .teams
        .get(team_idx)
        .map(|team| {
            team.char_team
                .iter()
                .filter_map(perlica_logic::character::char_bag::TeamSlot::object_id)
                .collect::<Vec<u64>>()
        })
        .unwrap_or_default();

    let result = ctx.player.scene.teleport(
        obj_id_list,
        Vector {
            x: position.x(),
            y: position.y(),
            z: position.z(),
        },
        Some(Vector {
            x: rotation.x(),
            y: rotation.y(),
            z: rotation.z(),
        }),
        common::time::now_ms() as u32,
        req.teleport_reason,
        None,
    );

    if let Err(e) = ctx.player.world.persist(&ctx.player.uid, ctx.db).await {
        warn!(
            "Failed to persist world after teleport: uid={}, error={}",
            ctx.player.uid, e
        );
    }

    result
}
