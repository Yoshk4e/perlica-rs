use crate::net::NetContext;
use perlica_logic::movement::MovementManager;
use perlica_logic::traits::{Positioned3D, SyncWriteBack};
use perlica_proto::{Code, CsMoveObjectMove, ScMoveObjectMove};

/// Maximum distance (world units) a player may move in a single packet.
///
/// At the reference sprint speed of 20 wu/s and a worst-case packet interval
/// of ~500 ms the expected displacement is ~10 wu.  The limit is set well
/// above that to accommodate dash skills, knock-back, and network jitter
/// without allowing obvious teleportation.  Anything beyond this threshold
/// is almost certainly a position-spoof and is silently discarded.
const MAX_MOVE_DELTA: f32 = 60.0;

/// Squared version to avoid the sqrt on the hot path.
const MAX_MOVE_DELTA_SQ: f32 = MAX_MOVE_DELTA * MAX_MOVE_DELTA;

/// Updates the team leader's server-side position. Only the leader is tracked; other positions are client-authoritative.
/// Echoed back with `server_notify: true` for future peer broadcasting.
pub async fn on_cs_move_object_move(
    ctx: &mut NetContext<'_>,
    req: CsMoveObjectMove,
) -> ScMoveObjectMove {
    let leader_objid = {
        let bag = &ctx.player.char_bag;
        let team = &bag.teams[bag.meta.curr_team_index as usize];
        team.leader_index.object_id()
    };

    if !ctx.player.movement_initialized {
        ctx.player.movement = MovementManager::from(&ctx.player.world);
        ctx.player.movement_initialized = true;
    }

    for info in &req.move_info {
        if info.objid == leader_objid {
            if let Some(motion) = &info.motion_info {
                if let Some(pos) = &motion.position {
                    if !pos.x.is_finite() || !pos.y.is_finite() || !pos.z.is_finite() {
                        tracing::warn!(
                            "Rejected non-finite position: uid={}, x={}, y={}, z={}",
                            ctx.player.uid,
                            pos.x,
                            pos.y,
                            pos.z
                        );
                        ctx.send_error(
                            Code::ErrSceneAoiobjectPosError,
                            "position contains non-finite value",
                        )
                        .await;
                        break;
                    }

                    let old = ctx.player.movement.position();
                    let dx = pos.x - old.0;
                    let dy = pos.y - old.1;
                    let dz = pos.z - old.2;
                    let dist_sq = dx * dx + dy * dy + dz * dz;

                    if dist_sq > MAX_MOVE_DELTA_SQ {
                        tracing::warn!(
                            "Movement delta exceeded cap: uid={}, delta={:.1} wu (cap={:.1}), \
                             from=({:.1},{:.1},{:.1}) to=({:.1},{:.1},{:.1})",
                            ctx.player.uid,
                            dist_sq.sqrt(),
                            MAX_MOVE_DELTA,
                            old.0,
                            old.1,
                            old.2,
                            pos.x,
                            pos.y,
                            pos.z,
                        );
                        break;
                    }

                    ctx.player.movement.update_position(pos.x, pos.y, pos.z);
                }

                if let Some(rot) = &motion.rotation {
                    if !rot.x.is_finite() || !rot.y.is_finite() || !rot.z.is_finite() {
                        tracing::warn!(
                            "Rejected non-finite rotation: uid={}, x={}, y={}, z={}",
                            ctx.player.uid,
                            rot.x,
                            rot.y,
                            rot.z
                        );
                        ctx.send_error(
                            Code::ErrSceneAoiobjectPosError,
                            "rotation contains non-finite value",
                        )
                        .await;
                        break;
                    }

                    ctx.player.movement.update_rotation(rot.x, rot.y, rot.z);
                }

                ctx.player.movement.write_back_into(&mut ctx.player.world);

                let pos = ctx.player.movement.position();
                let (enter_view, leave_view) = ctx.player.scene.update_visible_entities(
                    pos,
                    ctx.assets,
                    &mut ctx.player.entities,
                );

                if let Some(msg) = enter_view {
                    let _ = ctx.notify(msg).await;
                }

                if let Some(msg) = leave_view {
                    let _ = ctx.notify(msg).await;
                }
            }
            break;
        }
    }

    tracing::trace!(
        "Movement update received: uid={}, move_count={}",
        ctx.player.uid,
        req.move_info.len()
    );

    ScMoveObjectMove {
        move_info: req.move_info,
        server_notify: true,
    }
}
