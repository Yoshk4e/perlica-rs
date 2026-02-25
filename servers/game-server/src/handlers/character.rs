use crate::net::NetContext;
use perlica_proto::{CsCharSetBattleInfo, ScCharSyncStatus};
use tracing::{debug, instrument};

#[instrument(skip(ctx), fields(uid = %ctx.player.uid, objid = req.objid))]
pub async fn on_cs_char_set_battle_info(
    ctx: &mut NetContext<'_>,
    req: CsCharSetBattleInfo,
) -> ScCharSyncStatus {
    if let Some(bi) = &req.battle_info {
        debug!(
            hp = bi.hp,
            ultimate_sp = bi.ultimatesp,
            "battle info update"
        );
        ctx.player
            .char_bag
            .update_battle_info(req.objid, bi.hp, bi.ultimatesp);
    } else {
        debug!("battle info missing");
    }

    ScCharSyncStatus {
        objid: req.objid,
        is_dead: req
            .battle_info
            .as_ref()
            .map(|b| b.hp <= 0.0)
            .unwrap_or(false),
        battle_info: req.battle_info,
    }
}
