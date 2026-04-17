//! Battle-state synchronisation for characters.

use crate::net::NetContext;
use perlica_proto::CsCharSetBattleInfo;
use tracing::{debug, warn};

pub async fn on_cs_char_set_battle_info(ctx: &mut NetContext<'_>, req: CsCharSetBattleInfo) {
    debug!(
        "Battle info update: objid={}, has_battle_info={}",
        req.objid,
        req.battle_info.is_some()
    );
    if let Some(bi) = &req.battle_info {
        ctx.player
            .char_bag
            .update_battle_info(req.objid, bi.hp, bi.ultimatesp);
    } else {
        warn!(
            "Battle info update ignored: missing data for objid={}",
            req.objid
        );
    }
}
