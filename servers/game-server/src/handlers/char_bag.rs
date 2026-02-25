use crate::net::NetContext;
use tracing::{debug, error, instrument};

#[instrument(skip(ctx), fields(uid = %ctx.player.uid))]
pub async fn push_char_bag(ctx: &mut NetContext<'_>) -> bool {
    match ctx.player.char_bag.char_bag_info(ctx.player.resources) {
        Ok(msg) => {
            debug!(
                chars = msg.char_info.len(),
                teams = msg.team_info.len(),
                "char bag"
            );
            if let Err(e) = ctx.notify(msg).await {
                error!(error = %e, "char bag push failed");
                return false;
            }
            true
        }
        Err(e) => {
            error!(error = %e, "char bag info failed");
            false
        }
    }
}

#[instrument(skip(ctx), fields(uid = %ctx.player.uid))]
pub async fn push_char_attrs(ctx: &mut NetContext<'_>) -> bool {
    let msgs = ctx.player.char_bag.char_attrs(ctx.player.resources);
    debug!(count = msgs.len(), "char attrs");

    for msg in msgs {
        if let Err(e) = ctx.notify(msg).await {
            error!(error = %e, "char attrs push failed");
            return false;
        }
    }
    true
}

#[instrument(skip(ctx), fields(uid = %ctx.player.uid))]
pub async fn push_char_status(ctx: &mut NetContext<'_>) -> bool {
    let msgs = ctx.player.char_bag.char_status();
    debug!(count = msgs.len(), "char status");

    for msg in msgs {
        if let Err(e) = ctx.notify(msg).await {
            error!(error = %e, "char status push failed");
            return false;
        }
    }
    true
}
