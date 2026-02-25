use crate::net::NetContext;
use perlica_logic::enums::UnlockSystemType;
use perlica_proto::ScSyncAllUnlock;
use tracing::{debug, error, instrument};

#[instrument(skip(ctx), fields(uid = %ctx.player.uid))]
pub async fn push_unlocks(ctx: &mut NetContext<'_>) -> bool {
    let msg = ScSyncAllUnlock {
        unlock_systems: UnlockSystemType::all(),
    };
    debug!(count = msg.unlock_systems.len(), "unlocks");
    if let Err(e) = ctx.notify(msg).await {
        error!(error = %e, "unlocks push failed");
        return false;
    }
    true
}
