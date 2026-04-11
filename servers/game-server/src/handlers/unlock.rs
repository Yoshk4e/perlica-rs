use crate::net::NetContext;
use perlica_logic::enums::UnlockSystemType;
use perlica_proto::ScSyncAllUnlock;
use tracing::{debug, error};

/// Pushes `ScSyncAllUnlock` with every system unlocked. Called once during login.
pub async fn push_unlocks(ctx: &mut NetContext<'_>) -> bool {
    let msg = ScSyncAllUnlock {
        unlock_systems: UnlockSystemType::all(),
    };
    debug!(
        "unlocks: uid={}, count={}",
        ctx.player.uid,
        msg.unlock_systems.len()
    );
    if let Err(e) = ctx.notify(msg).await {
        error!("unlocks push failed: uid={}, error={}", ctx.player.uid, e);
        return false;
    }
    true
}
