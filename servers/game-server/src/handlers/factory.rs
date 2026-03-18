use crate::net::NetContext;
use perlica_proto::{ScFactorySyncContext, ScdFactorySyncRegion};
use tracing::{debug, error};

pub async fn push_factory(ctx: &mut NetContext<'_>) -> bool {
    let msg = ScFactorySyncContext {
        tms: 0,
        current_region: "test01".to_string(),
        regions: vec![ScdFactorySyncRegion {
            name: "test01".to_string(),
            blackboard: None,
            nodes: vec![],
            scenes: vec![],
        }],
        quickbars: vec![],
    };
    debug!(
        "factory: uid={}, regions={}",
        ctx.player.uid,
        msg.regions.len()
    );
    if let Err(e) = ctx.notify(msg).await {
        error!("factory push failed: uid={}, error={}", ctx.player.uid, e);
        return false;
    }
    true
}
