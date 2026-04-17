//! Intra- and inter-scene teleport.
//!
//! Inter-scene teleports wipe entities and reset level-script runtime;
//! intra-scene warps preserve all existing script state so puzzles etc.
//! don't get clobbered.

use crate::net::NetContext;
use perlica_proto::{CsSceneTeleport, ScSceneTeleport, Vector};
use tracing::debug;

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

    let position = req.position.unwrap_or(Vector {
        x: ctx.player.movement.pos_x,
        y: ctx.player.movement.pos_y,
        z: ctx.player.movement.pos_z,
    });
    ctx.player
        .movement
        .update_position(position.x, position.y, position.z);
    let rotation_vec = req.rotation.unwrap_or(Vector {
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
