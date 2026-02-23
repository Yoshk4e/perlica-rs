use crate::session::NetContext;
use perlica_proto::{CsMoveObjectMove, ScMoveObjectMove};
use tracing::debug;

pub async fn on_cs_move_object_move(
    ctx: &mut NetContext<'_>,
    req: CsMoveObjectMove,
) -> ScMoveObjectMove {
    for info in &req.move_info {
        debug!(
            "Move obj {} | pos={:?} rot={:?} speed={:?} state={}",
            info.objid,
            info.motion_info.as_ref().and_then(|m| m.position.as_ref()),
            info.motion_info.as_ref().and_then(|m| m.rotation.as_ref()),
            info.motion_info.as_ref().and_then(|m| m.speed.as_ref()),
            info.motion_info.as_ref().map(|m| m.state).unwrap_or(0),
        );
    }

    ScMoveObjectMove {
        move_info: req.move_info,
        server_notify: true,
    }
}
