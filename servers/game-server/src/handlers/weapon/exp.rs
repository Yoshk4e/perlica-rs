use crate::net::NetContext;
use perlica_logic::character::char_bag::handle_weapon_add_exp;
use perlica_proto::{CsWeaponAddExp, ScItemBagSyncModify, ScWeaponAddExp, ScdItemDepotModify};
use std::collections::HashMap;
use tracing::warn;

pub async fn on_cs_weapon_add_exp(ctx: &mut NetContext<'_>, req: CsWeaponAddExp) -> ScWeaponAddExp {
    let result = handle_weapon_add_exp(
        &mut ctx.player.char_bag,
        req.weaponid,
        &req.cost_item_id2_count,
        &req.cost_weapon_ids,
        ctx.assets,
    );

    let result = match result {
        Ok(r) => r,
        Err(e) => {
            warn!(
                "WeaponAddExp failed: uid={}, weapon={}, err={:?}",
                ctx.player.uid, req.weaponid, e
            );
            return ScWeaponAddExp {
                weaponid: req.weaponid,
                new_exp: 0,
                weapon_lv: 1,
            };
        }
    };

    // Notify the client of consumed items + dissolved fodder weapons.
    let mut depot = result.consumed.build_depot_map();
    if !result.removed_fodder.is_empty() {
        depot.insert(
            1i32,
            ScdItemDepotModify {
                items: HashMap::new(),
                inst_list: vec![],
                del_inst_list: result.removed_fodder,
            },
        );
    }
    if !depot.is_empty() {
        let _ = ctx
            .notify(ScItemBagSyncModify {
                depot,
                bag: None,
                factory_depot: None,
                cannot_destroy: HashMap::new(),
                use_blackboard: None,
                is_new: false,
            })
            .await;
    }

    if let Err(e) = ctx
        .db
        .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
        .await
    {
        warn!(
            "Failed to persist char_bag after weapon add exp: uid={}, error={}",
            ctx.player.uid, e
        );
    }

    result.response
}
