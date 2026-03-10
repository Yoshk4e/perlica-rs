use crate::net::NetContext;
use perlica_proto::{CsCharSetBattleInfo, ScCharSyncStatus};
use tracing::{debug, instrument};

pub async fn on_cs_char_set_battle_info(ctx: &mut NetContext<'_>, req: CsCharSetBattleInfo) {
    if let Some(bi) = &req.battle_info {
        ctx.player
            .char_bag
            .update_battle_info(req.objid, bi.hp, bi.ultimatesp);
    }
}
