use crate::net::NetContext;
use perlica_db::Persistable;
use perlica_logic::character::char_bag::handle_weapon_breakthrough;
use perlica_proto::{
    Code, CsWeaponBreakthrough, MoneyInfo, ScItemBagSyncModify, ScSyncWallet, ScWeaponBreakthrough,
};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Advances breakthrough level by one. Weapon must be at its current level cap.
pub async fn on_cs_weapon_breakthrough(
    ctx: &mut NetContext<'_>,
    req: CsWeaponBreakthrough,
) -> ScWeaponBreakthrough {
    debug!(
        "Weapon breakthrough request: uid={}, weapon_id={}",
        ctx.player.uid, req.weaponid
    );

    let result =
        match handle_weapon_breakthrough(&mut ctx.player.char_bag, req.weaponid, ctx.assets) {
            Ok(r) => r,
            Err(e) => {
                error!(
                    "Weapon breakthrough failed: uid={}, weapon_id={}, error={:?}",
                    ctx.player.uid, req.weaponid, e
                );
                return ScWeaponBreakthrough {
                    weaponid: req.weaponid,
                    breakthrough_lv: 0,
                };
            }
        };

    if result.gold_cost > 0 {
        if !ctx.player.wallet.try_deduct_gold(result.gold_cost) {
            warn!(
                "Weapon breakthrough rejected: insufficient gold, uid={}, cost={}, balance={}",
                ctx.player.uid, result.gold_cost, ctx.player.wallet.gold
            );
            ctx.send_error(Code::ErrCommonParamInvalid, "insufficient gold")
                .await;
            return ScWeaponBreakthrough {
                weaponid: req.weaponid,
                breakthrough_lv: 0,
            };
        }

        let _ = ctx
            .notify(ScSyncWallet {
                money_list: vec![MoneyInfo {
                    id: "item_gold".to_string(),
                    amount: ctx.player.wallet.gold,
                }],
            })
            .await;

        if let Err(e) = ctx.player.wallet.persist(&ctx.player.uid, ctx.db).await {
            warn!(
                "Failed to persist wallet after weapon breakthrough: uid={}, error={}",
                ctx.player.uid, e
            );
        }
    }

    let depot_modify = result.consumed.build_depot_map();

    if !depot_modify.is_empty() {
        let _ = ctx
            .notify(ScItemBagSyncModify {
                depot: depot_modify,
                bag: None,
                factory_depot: None,
                cannot_destroy: HashMap::new(),
                use_blackboard: None,
                is_new: false,
            })
            .await;
    }

    info!(
        "Weapon breakthrough success: uid={}, weapon_id={}, new_lv={}, gold_cost={}",
        ctx.player.uid, req.weaponid, result.response.breakthrough_lv, result.gold_cost
    );

    if let Err(e) = ctx
        .db
        .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
        .await
    {
        warn!(
            "Failed to persist char_bag after weapon breakthrough: uid={}, error={}",
            ctx.player.uid, e
        );
    }

    result.response
}
