use crate::net::NetContext;
use perlica_logic::enums::UnlockSystemType;
use perlica_proto::ScSyncAllUnlock;
use tracing::{debug, error};

/// Pushes the full unlock state as `ScSyncAllUnlock`.
///
/// Sends every [`UnlockSystemType`] variant as unlocked so all game systems are
/// accessible from the start. Called once during the login sequence.
///
/// Returns `false` if the send channel is closed.
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
