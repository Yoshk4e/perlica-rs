use crate::net::NetContext;
use perlica_logic::movement::MovementManager;
use perlica_proto::{CsMoveObjectMove, ScMoveObjectMove};
use tracing::debug;

/// Handles `CsMoveObjectMove` — the authoritative position update from the client.
///
/// Only the team leader's motion is tracked server-side; other character positions
/// are driven entirely by the client. The leader's new position and rotation are
/// stored in the [`MovementManager`] and synced back to [`WorldState`] so they
/// persist across sessions.
///
/// The packet is echoed back as-is with `server_notify: true` so that any
/// future peer-broadcasting path can re-use the same message.
pub async fn on_cs_move_object_move(
    ctx: &mut NetContext<'_>,
    req: CsMoveObjectMove,
) -> ScMoveObjectMove {
    let leader_objid = {
        let bag = &ctx.player.char_bag;
        let team = &bag.teams[bag.meta.curr_team_index as usize];
        team.leader_index.object_id()
    };

    if ctx.player.movement.position_tuple() == MovementManager::default().position_tuple() {
        ctx.player.movement = MovementManager::from_world(&ctx.player.world);
    }

    for info in &req.move_info {
        if info.objid == leader_objid {
            if let Some(motion) = &info.motion_info {
                if let Some(pos) = &motion.position {
                    ctx.player.movement.update_position(pos.x, pos.y, pos.z);
                }
                if let Some(rot) = &motion.rotation {
                    ctx.player.movement.update_rotation(rot.x, rot.y, rot.z);
                }

                ctx.player.movement.sync_to_world(&mut ctx.player.world);

                let pos = ctx.player.movement.position_tuple();
                let (enter_view, leave_view) = ctx.player.scene.update_visible_entities(
                    pos,
                    ctx.assets,
                    &mut ctx.player.entities,
                );

                if let Some(msg) = enter_view {
                    if let Err(error) = ctx.notify(msg).await {
                        tracing::error!(
                            "Failed to send dynamic enter view: uid={}, error={:?}",
                            ctx.player.uid,
                            error
                        );
                    }
                }

                if let Some(msg) = leave_view {
                    if let Err(error) = ctx.notify(msg).await {
                        tracing::error!(
                            "Failed to send dynamic leave view: uid={}, error={:?}",
                            ctx.player.uid,
                            error
                        );
                    }
                }
            }
            break;
        }
    }

    debug!(
        "Movement update received: uid={}, move_count={}",
        ctx.player.uid,
        req.move_info.len()
    );

    ScMoveObjectMove {
        move_info: req.move_info,
        server_notify: true,
    }
}
