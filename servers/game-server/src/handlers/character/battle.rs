//! Battle-state synchronisation for characters.
//!
//! HP / ultimate-SP updates arrive on every damage tick during combat,
//! which is the highest-frequency mutator in the game loop. We do
//! **not** persist here — `update_battle_info` flips the dirty flag
//! through `get_char_by_objid_mut`, and the session-level periodic
//! flusher (`PERSIST_INTERVAL` in `net::session`) takes care of
//! writing it to disk every ~30 s. A clean disconnect also flushes,
//! so the only loss window is an unclean crash.

use crate::net::NetContext;
use perlica_logic::character::char_bag::CharIndex;
use perlica_proto::{Code, CsCharSetBattleInfo};
use tracing::{debug, warn};

const MAX_ULTIMATE_SP: f32 = 1.0;

pub async fn on_cs_char_set_battle_info(ctx: &mut NetContext<'_>, req: CsCharSetBattleInfo) {
    debug!(
        "Battle info update: objid={}, has_battle_info={}",
        req.objid,
        req.battle_info.is_some()
    );

    let team_idx = ctx.player.char_bag.meta.curr_team_index as usize;
    let in_active_team = ctx
        .player
        .char_bag
        .teams
        .get(team_idx)
        .map(|team| {
            team.char_team
                .iter()
                .any(|slot| slot.object_id() == Some(req.objid))
        })
        .unwrap_or(false);

    if !in_active_team {
        warn!(
            "Rejected battle info update: objid={} not in active team (team_idx={})",
            req.objid, team_idx
        );
        ctx.send_error(
            Code::ErrCharNotFound,
            format!("objid {} not in active team", req.objid),
        )
        .await;
        return;
    }

    let Some(bi) = &req.battle_info else {
        warn!(
            "Battle info update ignored: missing data for objid={}",
            req.objid
        );
        return;
    };

    let char_idx = CharIndex::from_object_id(req.objid);
    let max_hp = ctx
        .player
        .char_bag
        .chars
        .get(char_idx.as_usize())
        .and_then(|c| {
            ctx.assets
                .characters
                .get_stats(&c.template_id, c.level, c.break_stage)
                .map(|a| a.hp)
        })
        .unwrap_or(f64::MAX); // If we can't resolve stats, don't clamp.

    let clamped_hp = bi.hp.clamp(0.0, max_hp);
    if clamped_hp != bi.hp {
        warn!(
            "HP clamped for objid={}: raw={}, clamped={}, max_hp={}",
            req.objid, bi.hp, clamped_hp, max_hp
        );
    }

    let clamped_sp = bi.ultimatesp.clamp(0.0, MAX_ULTIMATE_SP);
    if clamped_sp != bi.ultimatesp {
        warn!(
            "SP clamped for objid={}: raw={}, clamped={}, max_sp={}",
            req.objid, bi.ultimatesp, clamped_sp, MAX_ULTIMATE_SP
        );
    }

    ctx.player
        .char_bag
        .update_battle_info(req.objid, clamped_hp, clamped_sp);

    // no `.persist()` here - the dirty flag is enough.
}
