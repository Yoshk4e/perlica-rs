use crate::net::NetContext;
use perlica_proto::{ScFactorySyncContext, ScdFactorySyncRegion};
use tracing::{debug, error, instrument};

#[instrument(skip(ctx), fields(uid = %ctx.player.uid))]
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
    debug!(regions = msg.regions.len(), "factory");
    if let Err(e) = ctx.notify(msg).await {
        error!(error = %e, "factory push failed");
        return false;
    }
    true
}
