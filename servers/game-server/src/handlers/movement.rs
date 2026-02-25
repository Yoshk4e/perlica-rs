use crate::net::NetContext;
use perlica_proto::{CsMoveObjectMove, ScMoveObjectMove};
use tracing::{debug, instrument};

#[instrument(skip(_ctx), fields(move_count = req.move_info.len()))]
pub async fn on_cs_move_object_move(
    _ctx: &mut NetContext<'_>,
    req: CsMoveObjectMove,
) -> ScMoveObjectMove {
    debug!(count = req.move_info.len(), "movement update");
    ScMoveObjectMove {
        move_info: req.move_info,
        server_notify: true,
    }
}
