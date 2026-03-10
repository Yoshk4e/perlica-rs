use crate::net::NetContext;
use perlica_logic::character::char_bag::CharIndex;
use perlica_proto::{
    CsCharBagSetTeamLeader, CsCharSetBattleInfo, ScCharBagSetTeamLeader, ScCharSyncStatus,
};
use tracing::{debug, instrument};

pub async fn on_cs_char_set_battle_info(ctx: &mut NetContext<'_>, req: CsCharSetBattleInfo) {
    if let Some(bi) = &req.battle_info {
        ctx.player
            .char_bag
            .update_battle_info(req.objid, bi.hp, bi.ultimatesp);
    }
}

pub async fn on_cs_char_bag_set_team_leader(
    ctx: &mut NetContext<'_>,
    req: CsCharBagSetTeamLeader,
) -> ScCharBagSetTeamLeader {
    let team_idx = req.team_index as usize;
    if let Some(team) = ctx.player.char_bag.teams.get_mut(team_idx) {
        team.leader_index = CharIndex::from_object_id(req.leaderid);
    }
    ScCharBagSetTeamLeader {
        team_index: req.team_index,
        leaderid: req.leaderid,
    }
}
