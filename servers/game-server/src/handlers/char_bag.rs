use crate::session::NetContext;
use perlica_logic::enums::UnlockSystemType;
use perlica_proto::ScSyncAllUnlock;
use tracing::debug;

pub async fn send_char_bag_sync(ctx: &mut NetContext<'_>) -> bool {
    match crate::player::char_bag::prepare_char_bag_sync(ctx.player) {
        Ok(msg) => {
            debug!(
                "CharBagSync: {} chars, {} teams",
                msg.char_info.len(),
                msg.team_info.len()
            );
            ctx.notify(msg).await.is_ok()
        }
        Err(e) => {
            tracing::error!("CharBagSync failed: {}", e);
            false
        }
    }
}

pub async fn send_unlock_sync(ctx: &mut NetContext<'_>) -> bool {
    let msg = ScSyncAllUnlock {
        unlock_systems: UnlockSystemType::default_unlocked(),
    };
    debug!("UnlockSync: {} systems", msg.unlock_systems.len());
    ctx.notify(msg).await.is_ok()
}
