use crate::net::NetContext;
use perlica_logic::character::char_bag::{CharIndex, Team, TeamSlot};
use perlica_proto::{
    CsCharBagSetCurrTeamIndex, CsCharBagSetTeam, CsCharBagSetTeamLeader, CsCharBagSetTeamName,
    CsCharBreak, CsCharLevelUp, CsCharSetBattleInfo, CsCharSetNormalSkill, CsCharSetTeamSkill,
    CsCharSkillLevelUp, MoneyInfo, ScCharBagSetCurrTeamIndex, ScCharBagSetTeam,
    ScCharBagSetTeamLeader, ScCharBagSetTeamName, ScCharBreak, ScCharLevelUp, ScCharSetNormalSkill,
    ScCharSetTeamSkill, ScCharSkillLevelUp, ScCharSyncLevelExp, ScItemBagSyncModify, ScSyncWallet,
    ScdItemDepotModify, SkillLevelInfo,
};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Cumulative exp required to reach `target_level` from level 1.
fn cumulative_exp(level_up_exp: &[i32], target_level: i32) -> i64 {
    let mut total = 0i64;
    for i in 0..(target_level - 1) as usize {
        let cost = level_up_exp.get(i).copied().unwrap_or(0);
        if cost < 0 {
            break;
        }
        total += cost as i64;
    }
    total
}

/// Advances `current_level` as far as possible given `new_total_exp`, capped at `max_level`.
/// Returns `(achieved_level, remaining_exp_within_that_level)`.
fn calculate_level_from_total_exp(
    level_up_exp: &[i32],
    current_level: i32,
    new_total_exp: i64,
    max_level: i32,
) -> (i32, i32) {
    let mut lv = current_level;
    loop {
        if lv >= max_level {
            break;
        }
        let cost = level_up_exp.get(lv as usize - 1).copied().unwrap_or(-1);
        if cost < 0 {
            break;
        }
        let cum_next = cumulative_exp(level_up_exp, lv + 1);
        if new_total_exp >= cum_next {
            lv += 1;
        } else {
            break;
        }
    }
    let cum_at_lv = cumulative_exp(level_up_exp, lv);
    let remaining = (new_total_exp - cum_at_lv).max(0) as i32;
    (lv, remaining)
}

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
    super::char_bag::push_char_status_for_ids(ctx, &new_ids).await;
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
        super::char_bag::push_char_status_for_ids(ctx, &req.char_team).await;
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

/// Consumes exp items and advances the character's level.
pub async fn on_cs_char_level_up(ctx: &mut NetContext<'_>, req: CsCharLevelUp) -> ScCharLevelUp {
    debug!(
        "CharLevelUp: uid={}, char_id={}, items={}",
        ctx.player.uid,
        req.char_obj_id,
        req.items.len()
    );

    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) else {
        warn!("CharLevelUp failed: unknown char_id={}", req.char_obj_id);
        return ScCharLevelUp {
            char_obj_id: req.char_obj_id,
        };
    };

    let template_id = char_data.template_id.clone();
    let break_stage = char_data.break_stage;
    let current_level = char_data.level;
    let current_exp = char_data.exp as i64;

    let max_level = ctx
        .assets
        .characters
        .get(&template_id)
        .and_then(|c| {
            c.break_data
                .iter()
                .find(|bd| bd.break_stage == break_stage)
                .map(|bd| bd.max_level as i32)
        })
        .unwrap_or(current_level);

    if current_level >= max_level {
        return ScCharLevelUp {
            char_obj_id: req.char_obj_id,
        };
    }

    let level_up_exp = ctx.assets.characters.char_const().level_up_exp.as_slice();

    // Exp the character still needs to advance from their current level.
    let cum_at_current = cumulative_exp(level_up_exp, current_level);
    let exp_to_next_level = {
        let cum_next = cumulative_exp(level_up_exp, current_level + 1);
        (cum_next - cum_at_current - current_exp).max(0)
    };

    let mut total_exp_gained: i64 = 0;
    let mut consumed: HashMap<String, u32> = HashMap::new();

    use config::item::ItemDepotType;

    for item_info in &req.items {
        if item_info.res_count <= 0 {
            continue;
        }
        let count = item_info.res_count as u32;

        let exp_per_unit = ctx.assets.items.char_exp_for_item(&item_info.res_id);

        if exp_per_unit == 0 {
            warn!(
                "CharLevelUp: item {} gives 0 exp, skipping",
                item_info.res_id
            );
            continue;
        }

        let consumed_ok = ctx
            .player
            .char_bag
            .item_manager
            .consume_stackable(ItemDepotType::SpecialItem, &item_info.res_id, count)
            .is_ok()
            || ctx
                .player
                .char_bag
                .item_manager
                .consume_stackable(ItemDepotType::Factory, &item_info.res_id, count)
                .is_ok();

        if consumed_ok {
            total_exp_gained += exp_per_unit * count as i64;
            *consumed.entry(item_info.res_id.clone()).or_insert(0) += count;
        } else {
            warn!(
                "CharLevelUp: could not consume {} × {}",
                count, item_info.res_id
            );
        }
    }

    if total_exp_gained == 0 {
        return ScCharLevelUp {
            char_obj_id: req.char_obj_id,
        };
    }

    let new_total_exp = cum_at_current + current_exp + total_exp_gained;
    let (new_level, remaining_exp) =
        calculate_level_from_total_exp(level_up_exp, current_level, new_total_exp, max_level);

    let at_max = new_level >= max_level;
    let synced_exp = if at_max { 0 } else { remaining_exp };

    let char_data = ctx
        .player
        .char_bag
        .get_char_by_objid_mut(req.char_obj_id)
        .unwrap();
    char_data.level = new_level;
    char_data.exp = synced_exp;

    if let Some(attrs) = ctx
        .assets
        .characters
        .get_stats(&template_id, new_level, break_stage)
    {
        char_data.hp = attrs.hp;
    }

    info!(
        "CharLevelUp complete: uid={}, char_id={}, level {}→{}, exp_gained={}, remaining={}",
        ctx.player.uid, req.char_obj_id, current_level, new_level, total_exp_gained, synced_exp
    );

    if let Some(attr_msg) = ctx
        .player
        .char_bag
        .char_attrs(ctx.assets)
        .into_iter()
        .find(|a| a.obj_id == req.char_obj_id)
    {
        if let Err(e) = ctx.notify(attr_msg).await {
            error!("Failed to sync attrs after level up: {:?}", e);
        }
    }

    if let Err(e) = ctx
        .notify(ScCharSyncLevelExp {
            char_obj_id: req.char_obj_id,
            level: new_level,
            exp: synced_exp,
        })
        .await
    {
        error!("Failed to sync level/exp: {:?}", e);
    }

    if !consumed.is_empty() {
        let items: HashMap<String, i64> = consumed
            .keys()
            .map(|id| {
                let in_special = ctx
                    .player
                    .char_bag
                    .item_manager
                    .count_of(ItemDepotType::SpecialItem, id);
                let new_count = if ctx.player.char_bag.item_manager.has_stackable(
                    ItemDepotType::SpecialItem,
                    id,
                    0,
                ) {
                    in_special
                } else {
                    ctx.player
                        .char_bag
                        .item_manager
                        .count_of(ItemDepotType::Factory, id)
                };
                (id.clone(), new_count as i64)
            })
            .collect();

        let mut depot_modify = HashMap::new();
        depot_modify.insert(
            4i32,
            ScdItemDepotModify {
                items,
                inst_list: vec![],
                del_inst_list: vec![],
            },
        );

        if let Err(e) = ctx
            .notify(ScItemBagSyncModify {
                depot: depot_modify,
                bag: None,
                factory_depot: None,
                cannot_destroy: HashMap::new(),
                use_blackboard: None,
                is_new: false,
            })
            .await
        {
            error!("Failed to send item bag modify: {:?}", e);
        }
    }

    ScCharLevelUp {
        char_obj_id: req.char_obj_id,
    }
}

/// Advances the character's break stage aka ascention.
pub async fn on_cs_char_break(ctx: &mut NetContext<'_>, req: CsCharBreak) -> ScCharBreak {
    debug!(
        "CharBreak: uid={}, char_id={}, from_stage={}",
        ctx.player.uid, req.char_obj_id, req.stage
    );
    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) else {
        warn!("CharBreak failed: unknown char_id={}", req.char_obj_id);
        return ScCharBreak {
            char_obj_id: req.char_obj_id,
            stage: 0,
        };
    };
    let template_id = char_data.template_id.clone();
    let from_stage = req.stage as u32;

    if from_stage == char_data.break_stage {
        let new_stage = char_data.break_stage + 1;
        char_data.break_stage = new_stage;
        if let Some(attrs) =
            ctx.assets
                .characters
                .get_stats(&template_id, char_data.level, new_stage)
        {
            char_data.hp = attrs.hp;
        }
        info!(
            "CharBreak complete: uid={}, char_id={}, stage {} → {}",
            ctx.player.uid, req.char_obj_id, from_stage, new_stage
        );
    } else {
        warn!(
            "CharBreak rejected: current={}, requested from={}",
            char_data.break_stage, from_stage
        );
    }

    let confirmed = char_data.break_stage as i32;
    if let Some(attr_msg) = ctx
        .player
        .char_bag
        .char_attrs(ctx.assets)
        .into_iter()
        .find(|a| a.obj_id == req.char_obj_id)
    {
        let _ = ctx.notify(attr_msg).await;
    }
    ScCharBreak {
        char_obj_id: req.char_obj_id,
        stage: confirmed,
    }
}

pub async fn on_cs_char_set_normal_skill(
    ctx: &mut NetContext<'_>,
    req: CsCharSetNormalSkill,
) -> ScCharSetNormalSkill {
    if let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) {
        char_data
            .skill_levels
            .entry(req.normal_skillid.clone())
            .or_insert(1);
    }
    ScCharSetNormalSkill {
        char_obj_id: req.char_obj_id,
        normal_skillid: req.normal_skillid,
    }
}

pub async fn on_cs_char_skill_level_up(
    ctx: &mut NetContext<'_>,
    req: CsCharSkillLevelUp,
) -> ScCharSkillLevelUp {
    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.objid) else {
        return ScCharSkillLevelUp {
            objid: req.objid,
            level_info: None,
        };
    };
    let template_id = char_data.template_id.clone();
    let bundles = ctx.assets.char_skills.get_char_skills(&template_id);
    let max_level = bundles
        .iter()
        .find(|b| b.entries.iter().any(|e| e.skill_id == req.skill_id))
        .and_then(|b| b.entries.iter().map(|e| e.level).max())
        .unwrap_or(1);
    let current = char_data
        .skill_levels
        .get(&req.skill_id)
        .copied()
        .unwrap_or(1);
    let new_level = (current + 1).min(max_level);
    char_data
        .skill_levels
        .insert(req.skill_id.clone(), new_level);
    info!(
        "SkillLevelUp: uid={}, char_id={}, skill={}, lv={}",
        ctx.player.uid, req.objid, req.skill_id, new_level
    );
    ScCharSkillLevelUp {
        objid: req.objid,
        level_info: Some(SkillLevelInfo {
            skill_id: req.skill_id,
            skill_level: new_level as i32,
            skill_max_level: max_level as i32,
        }),
    }
}

pub async fn on_cs_char_set_team_skill(
    ctx: &mut NetContext<'_>,
    req: CsCharSetTeamSkill,
) -> ScCharSetTeamSkill {
    if let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.objid) {
        char_data
            .skill_levels
            .entry(req.normal_skillid.clone())
            .or_insert(1);
    }
    ScCharSetTeamSkill {
        objid: req.objid,
        team_idx: req.team_idx,
        normal_skillid: req.normal_skillid,
    }
}
