use crate::net::NetContext;
use perlica_proto::{MoneyInfo, ScSyncWallet};
use tracing::debug;

pub async fn push_wallet(ctx: &mut NetContext<'_>) -> bool {
    debug!(
        "Pushing wallet: uid={}, gold={}, diamond={}",
        ctx.player.uid, ctx.player.wallet.gold, ctx.player.wallet.diamond
    );

    ctx.notify(ScSyncWallet {
        money_list: vec![
            MoneyInfo {
                id: "item_gold".to_string(),
                amount: ctx.player.wallet.gold,
            },
            MoneyInfo {
                id: "item_diamond".to_string(),
                amount: ctx.player.wallet.diamond,
            },
        ],
    })
    .await
    .is_ok()
}
