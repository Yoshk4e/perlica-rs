use crate::net::NetContext;
use perlica_proto::{CsMoveObjectMove, ScMoveObjectMove};
use tracing::debug;

pub async fn on_cs_move_object_move(
    ctx: &mut NetContext<'_>,
    req: CsMoveObjectMove,
) -> ScMoveObjectMove {
    // Track the leader's position so it persists on disconnect.
    let leader_objid = {
        let bag = &ctx.player.char_bag;
        let team = &bag.teams[bag.meta.curr_team_index as usize];
        team.leader_index.object_id()
    };

    for info in &req.move_info {
        if info.objid == leader_objid {
            if let Some(motion) = &info.motion_info {
                if let Some(pos) = &motion.position {
                    ctx.player.world.pos_x = pos.x;
                    ctx.player.world.pos_y = pos.y;
                    ctx.player.world.pos_z = pos.z;
                }
                if let Some(rot) = &motion.rotation {
                    ctx.player.world.rot_x = rot.x;
                    ctx.player.world.rot_y = rot.y;
                    ctx.player.world.rot_z = rot.z;
                }
            }
            break;
        }
    }

    debug!(count = req.move_info.len(), "movement update");
    ScMoveObjectMove {
        move_info: req.move_info,
        server_notify: true,
    }
}
