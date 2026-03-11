use crate::net::NetContext;
use common::time::now_ms;
use perlica_proto::{CsPing, ScPing};
use tracing::debug;

pub async fn on_csping(_ctx: &mut NetContext<'_>, req: CsPing) -> ScPing {
    let server_ts = now_ms();
    debug!(server_ts, "ping");
    ScPing {
        client_ts: req.client_ts,
        server_ts,
    }
}
