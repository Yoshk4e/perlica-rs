//! Team composition handlers: set leader, switch active team, set members, rename.

use crate::net::NetContext;
use perlica_logic::character::char_bag::{CharIndex, Team, TeamSlot};
use perlica_proto::{
    CsCharBagSetCurrTeamIndex, CsCharBagSetTeam, CsCharBagSetTeamLeader, CsCharBagSetTeamName,
    ScCharBagSetCurrTeamIndex, ScCharBagSetTeam, ScCharBagSetTeamLeader, ScCharBagSetTeamName,
};
use tracing::{debug, error, warn};

pub async fn on_cs_char_bag_set_team_leader(
    ctx: &mut NetContext<'_>,
    req: CsCharBagSetTeamLeader,
) -> ScCharBagSetTeamLeader {
    debug!(
        "Set team leader request: team_index={}, leader_id={}",
        req.team_index, req.leaderid
    );
    let team_idx = req.team_index as usize;
    if let Some(team) = ctx.player.char_bag.teams.get_mut(team_idx) {
        let in_team = team.char_team.iter().any(|s| {
            s.char_index()
                .map(|i| i.object_id() == req.leaderid)
                .unwrap_or(false)
        });
        if in_team {
            team.leader_index = CharIndex::from_object_id(req.leaderid);
        } else {
            warn!(
                "Rejected team leader update: leader_id={} not in team",
                req.leaderid
            );
        }
    }
    ScCharBagSetTeamLeader {
        team_index: req.team_index,
        leaderid: req.leaderid,
    }
}

pub async fn on_cs_char_bag_set_curr_team_index(
    ctx: &mut NetContext<'_>,
    req: CsCharBagSetCurrTeamIndex,
) {
    let old = ctx.player.char_bag.meta.curr_team_index as usize;
    let new = req.team_index as usize;
    if new >= ctx.player.char_bag.teams.len() {
        let _ = ctx
            .send(ScCharBagSetCurrTeamIndex {
                team_index: old as i32,
            })
            .await;
        return;
    }
    let old_ids: Vec<u64> = ctx.player.char_bag.teams[old]
        .char_team
        .iter()
        .filter_map(|s| s.object_id())
        .collect();
    let new_ids: Vec<u64> = ctx.player.char_bag.teams[new]
        .char_team
        .iter()
        .filter_map(|s| s.object_id())
        .collect();
    ctx.player.char_bag.meta.curr_team_index = new as u32;
    if let Err(e) = ctx
        .send(ScCharBagSetCurrTeamIndex {
            team_index: req.team_index,
        })
        .await
    {
        error!("Failed to ack team index change: {:?}", e);
        return;
    }
    let (leave, enter, self_info) = ctx.player.scene.handle_team_index_switch(
        &old_ids,
        &new_ids,
        &ctx.player.char_bag,
        &ctx.player.movement,
        ctx.assets,
        &mut ctx.player.entities,
    );
    if let Some(l) = leave {
        let _ = ctx.notify(l).await;
    }
    let _ = ctx.notify(enter).await;
    let _ = ctx.notify(self_info).await;
    crate::handlers::char_bag::push_char_status_for_ids(ctx, &new_ids).await;
}

pub async fn on_cs_char_bag_set_team(ctx: &mut NetContext<'_>, req: CsCharBagSetTeam) {
    let uid = ctx.player.uid.clone();
    let team_index = req.team_index as usize;
    if team_index >= ctx.player.char_bag.teams.len() {
        let _ = ctx
            .send(ScCharBagSetTeam {
                team_index: req.team_index,
                char_team: vec![],
            })
            .await;
        return;
    }
    let old_ids: Vec<u64> = ctx.player.char_bag.teams[team_index]
        .char_team
        .iter()
        .filter_map(|s| s.object_id())
        .collect();
    let is_active = team_index == ctx.player.char_bag.meta.curr_team_index as usize;
    let mut new_slots: [TeamSlot; Team::SLOTS_COUNT] = Default::default();
    for (i, &objid) in req.char_team.iter().enumerate().take(Team::SLOTS_COUNT) {
        new_slots[i] = if objid == 0 {
            TeamSlot::Empty
        } else {
            TeamSlot::Occupied(CharIndex::from_object_id(objid))
        };
    }
    ctx.player.char_bag.teams[team_index].char_team = new_slots;

    {
        let team = &mut ctx.player.char_bag.teams[team_index];
        let leader_still_in_team = team
            .char_team
            .iter()
            .filter_map(|s| s.char_index())
            .any(|idx| idx == team.leader_index);

        if !leader_still_in_team {
            team.leader_index = team
                .char_team
                .iter()
                .find_map(|s| s.char_index())
                .unwrap_or_default();
        }
    }

    if let Err(e) = ctx
        .send(ScCharBagSetTeam {
            team_index: req.team_index,
            char_team: req.char_team.clone(),
        })
        .await
    {
        error!("Failed to ack set team: uid={}, {:?}", uid, e);
        return;
    }
    if is_active {
        let (leave, enter, self_info) = ctx.player.scene.handle_active_team_update(
            &old_ids,
            &req.char_team,
            &ctx.player.char_bag,
            &ctx.player.movement,
            ctx.assets,
            &mut ctx.player.entities,
        );
        if let Some(l) = leave {
            let _ = ctx.notify(l).await;
        }
        let _ = ctx.notify(enter).await;
        let _ = ctx.notify(self_info).await;
        crate::handlers::char_bag::push_char_status_for_ids(ctx, &req.char_team).await;
    } else {
        let self_info = ctx.player.scene.handle_inactive_team_update(
            &req.char_team,
            &ctx.player.char_bag,
            &ctx.player.movement,
            ctx.assets,
            &ctx.player.entities,
        );
        let _ = ctx.notify(self_info).await;
    }
}

pub async fn on_cs_char_bag_set_team_name(
    ctx: &mut NetContext<'_>,
    req: CsCharBagSetTeamName,
) -> ScCharBagSetTeamName {
    let team_index = req.team_index as usize;
    if let Some(team) = ctx.player.char_bag.teams.get_mut(team_index) {
        team.name = req.team_name.clone();
    } else {
        return ScCharBagSetTeamName {
            team_index: req.team_index,
            team_name: String::new(),
        };
    }
    ScCharBagSetTeamName {
        team_index: req.team_index,
        team_name: req.team_name,
    }
}
