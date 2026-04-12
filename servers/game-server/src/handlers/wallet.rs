use crate::net::NetContext;
use perlica_logic::item::{WALLET_DIAMOND_AMOUNT, WALLET_GOLD_AMOUNT};
use perlica_proto::{MoneyInfo, ScSyncWallet};
use tracing::debug;

/// Pushes `ScSyncWallet` with starter amounts for gold and diamonds.
/// Returns `false` if the send fails.
pub async fn push_wallet(ctx: &mut NetContext<'_>) -> bool {
    debug!("Pushing wallet: uid={}", ctx.player.uid);

    ctx.notify(ScSyncWallet {
        money_list: vec![
            MoneyInfo {
                id: "item_gold".to_string(),
                amount: WALLET_GOLD_AMOUNT,
            },
            MoneyInfo {
                id: "item_diamond".to_string(),
                amount: WALLET_DIAMOND_AMOUNT,
            },
        ],
    })
    .await
    .is_ok()
}
