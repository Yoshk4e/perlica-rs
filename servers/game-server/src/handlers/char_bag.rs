use crate::net::NetContext;
use tracing::{debug, error};

pub async fn push_char_bag(ctx: &mut NetContext<'_>) -> bool {
    match ctx.player.char_bag.char_bag_info(ctx.assets) {
        Ok(msg) => {
            debug!(
                uid = %ctx.player.uid,
                chars = msg.char_info.len(),
                teams = msg.team_info.len(),
                "push char bag"
            );
            ctx.notify(msg).await.is_ok()
        }
        Err(e) => {
            error!(uid = %ctx.player.uid, error = %e, "char bag info failed");
            false
        }
    }
}

pub async fn push_item_bag_sync(ctx: &mut NetContext<'_>) -> bool {
    let msg = ctx.player.char_bag.item_bag_sync(ctx.assets);
    debug!(uid = %ctx.player.uid, "push item bag sync");
    ctx.notify(msg).await.is_ok()
}

pub async fn push_char_attrs(ctx: &mut NetContext<'_>) -> bool {
    let msgs = ctx.player.char_bag.char_attrs(ctx.assets);
    debug!(uid = %ctx.player.uid, count = msgs.len(), "push char attrs");
    for msg in msgs {
        if let Err(e) = ctx.notify(msg).await {
            error!(uid = %ctx.player.uid, error = %e, "char attrs push failed");
            return false;
        }
    }
    true
}

pub async fn push_char_status(ctx: &mut NetContext<'_>) -> bool {
    let msgs = ctx.player.char_bag.char_status();
    debug!(uid = %ctx.player.uid, count = msgs.len(), "push char status");
    for msg in msgs {
        if let Err(e) = ctx.notify(msg).await {
            error!(uid = %ctx.player.uid, error = %e, "char status push failed");
            return false;
        }
    }
    true
}
