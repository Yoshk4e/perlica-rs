use crate::session::NetContext;
use perlica_proto::{CsCharSetBattleInfo, ScCharSyncStatus};
use tracing::debug;

pub async fn on_cs_char_set_battle_info(
    ctx: &mut NetContext<'_>,
    req: CsCharSetBattleInfo,
) -> ScCharSyncStatus {
    debug!("CsCharSetBattleInfo for objid {}", req.objid);

    if let Some(bi) = &req.battle_info {
        ctx.player
            .char_bag
            .update_battle_info(req.objid, bi.hp, bi.ultimatesp);
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
