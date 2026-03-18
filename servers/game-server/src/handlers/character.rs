use crate::net::NetContext;
use perlica_logic::character::char_bag::{CharIndex, Team, TeamSlot};
use perlica_proto::{
    CsCharBagSetCurrTeamIndex, CsCharBagSetTeam, CsCharBagSetTeamLeader, CsCharBagSetTeamName,
    CsCharBreak, CsCharLevelUp, CsCharSetBattleInfo, CsCharSetNormalSkill, CsCharSetTeamSkill,
    CsCharSkillLevelUp, ScCharBagSetCurrTeamIndex, ScCharBagSetTeam, ScCharBagSetTeamLeader,
    ScCharBagSetTeamName, ScCharBreak, ScCharLevelUp, ScCharSetNormalSkill, ScCharSetTeamSkill,
    ScCharSkillLevelUp, ScCharSyncLevelExp, SkillLevelInfo,
};
use tracing::{debug, error, info, warn};

pub async fn on_cs_char_set_battle_info(ctx: &mut NetContext<'_>, req: CsCharSetBattleInfo) {
    debug!(
        "battle info update: objid={}, has_battle_info={}",
        req.objid,
        req.battle_info.is_some()
    );

    if let Some(bi) = &req.battle_info {
        ctx.player
            .char_bag
            .update_battle_info(req.objid, bi.hp, bi.ultimatesp);
        debug!(
            "battle info updated: objid={}, hp={}, ultimate_sp={}",
            req.objid, bi.hp, bi.ultimatesp
        );
    } else {
        warn!("battle info update missing data: objid={}", req.objid);
    }
}

pub async fn on_cs_char_bag_set_team_leader(
    ctx: &mut NetContext<'_>,
    req: CsCharBagSetTeamLeader,
) -> ScCharBagSetTeamLeader {
    debug!(
        "set team leader request: team_index={}, leader_id={}",
        req.team_index, req.leaderid
    );

    let team_idx = req.team_index as usize;

    if let Some(team) = ctx.player.char_bag.teams.get_mut(team_idx) {
        let leader_in_team = team.char_team.iter().any(|slot| {
            slot.char_index()
                .map(|idx| idx.object_id() == req.leaderid)
                .unwrap_or(false)
        });

        if leader_in_team {
            team.leader_index = CharIndex::from_object_id(req.leaderid);
            info!(
                "team leader updated: team_index={}, leader_id={}",
                req.team_index, req.leaderid
            );
        } else {
            warn!(
                "attempted to set leader not in team: team_index={}, leader_id={}",
                req.team_index, req.leaderid
            );
        }
    } else {
        error!("invalid team index: team_index={}", req.team_index);
    }

    ScCharBagSetTeamLeader {
        team_index: req.team_index,
        leaderid: req.leaderid,
    }
}

/// Switches the player's active team to a different slot.
///
/// Sends the `ScCharBagSetCurrTeamIndex` ACK **first**, then the scene diff
/// (leave/enter/self_info) for the same reason as [`on_cs_char_bag_set_team`].
pub async fn on_cs_char_bag_set_curr_team_index(
    ctx: &mut NetContext<'_>,
    req: CsCharBagSetCurrTeamIndex,
) {
    debug!(
        "set current team index request: new_team_index={}",
        req.team_index
    );

    let old_team_index = ctx.player.char_bag.meta.curr_team_index as usize;
    let new_team_index = req.team_index as usize;

    if new_team_index >= ctx.player.char_bag.teams.len() {
        error!(
            "invalid team index: team_index={}, max_teams={}",
            req.team_index,
            ctx.player.char_bag.teams.len()
        );
        let _ = ctx
            .send(ScCharBagSetCurrTeamIndex {
                team_index: old_team_index as i32,
            })
            .await;
        return;
    }

    if old_team_index == new_team_index {
        debug!("team index unchanged, skipping");
        let _ = ctx
            .send(ScCharBagSetCurrTeamIndex {
                team_index: req.team_index,
            })
            .await;
        return;
    }

    let old_team_ids: Vec<u64> = ctx.player.char_bag.teams[old_team_index]
        .char_team
        .iter()
        .filter_map(|slot| slot.object_id())
        .collect();

    let new_team_ids: Vec<u64> = ctx.player.char_bag.teams[new_team_index]
        .char_team
        .iter()
        .filter_map(|slot| slot.object_id())
        .collect();

    ctx.player.char_bag.meta.curr_team_index = new_team_index as u32;

    info!(
        "team index changed: old={}, new={}, old_count={}, new_count={}",
        old_team_index,
        new_team_index,
        old_team_ids.len(),
        new_team_ids.len()
    );

    // ACK first.
    if let Err(e) = ctx
        .send(ScCharBagSetCurrTeamIndex {
            team_index: req.team_index,
        })
        .await
    {
        error!("team index ack failed: {e}");
        return;
    }

    let (leave_view, enter_view, self_info) = ctx.player.scene.handle_team_index_switch(
        &old_team_ids,
        &new_team_ids,
        &ctx.player.char_bag,
        &ctx.player.movement,
        ctx.assets,
        &mut ctx.player.entities,
    );

    if let Some(leave) = leave_view {
        if let Err(e) = ctx.notify(leave).await {
            error!("team switch leave view failed: {e}");
        }
    }
    if let Err(e) = ctx.notify(enter_view).await {
        error!("team switch enter view failed: {e}");
    }
    if let Err(e) = ctx.notify(self_info).await {
        error!("team switch self info failed: {e}");
    }

    super::char_bag::push_char_status_for_ids(ctx, &new_team_ids).await;
}

/// Sets the composition of a team slot.
///
/// Sends the `ScCharBagSetTeam` ACK **first** so the client knows the new
/// composition before processing the scene update.
pub async fn on_cs_char_bag_set_team(ctx: &mut NetContext<'_>, req: CsCharBagSetTeam) {
    debug!(
        uid = %ctx.player.uid,
        team_index = req.team_index,
        char_count = req.char_team.len(),
        "set team composition request"
    );

    let team_index = req.team_index as usize;

    if team_index >= ctx.player.char_bag.teams.len() {
        error!(uid = %ctx.player.uid, team_index = req.team_index, "invalid team index");
        // ACK with empty char_team to signal rejection.
        let _ = ctx
            .send(ScCharBagSetTeam {
                team_index: req.team_index,
                char_team: vec![],
            })
            .await;
        return;
    }

    let old_team_ids: Vec<u64> = ctx.player.char_bag.teams[team_index]
        .char_team
        .iter()
        .filter_map(|slot| slot.object_id())
        .collect();

    let is_active_team = team_index == ctx.player.char_bag.meta.curr_team_index as usize;

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
        error!(uid = %ctx.player.uid, error = %e, "set team ack failed");
        return;
    }

    if is_active_team {
        let (leave_view, enter_view, self_info) = ctx.player.scene.handle_active_team_update(
            &old_team_ids,
            &req.char_team,
            &ctx.player.char_bag,
            &ctx.player.movement,
            ctx.assets,
            &mut ctx.player.entities,
        );

        if let Some(leave) = leave_view {
            if let Err(e) = ctx.notify(leave).await {
                error!(uid = %ctx.player.uid, error = %e, "team leave view failed");
            }
        }
        if let Err(e) = ctx.notify(enter_view).await {
            error!(uid = %ctx.player.uid, error = %e, "team enter view failed");
        }
        if let Err(e) = ctx.notify(self_info).await {
            error!(uid = %ctx.player.uid, error = %e, "team self info failed");
        }

        super::char_bag::push_char_status_for_ids(ctx, &req.char_team).await;
    } else {
        let self_info = ctx.player.scene.handle_inactive_team_update(
            &req.char_team,
            &ctx.player.char_bag,
            &ctx.player.movement,
            ctx.assets,
            &ctx.player.entities,
        );
        if let Err(e) = ctx.notify(self_info).await {
            error!(uid = %ctx.player.uid, error = %e, "inactive team update failed");
        }
    }
}

/// Renames a team slot.
///
/// Echoes the new name back via [`ScCharBagSetTeamName`]. If the index is out of
/// range the name is not applied and the response carries an empty string so the
/// client can detect the rejection.
pub async fn on_cs_char_bag_set_team_name(
    ctx: &mut NetContext<'_>,
    req: CsCharBagSetTeamName,
) -> ScCharBagSetTeamName {
    debug!(
        uid = %ctx.player.uid,
        team_index = req.team_index,
        name = %req.team_name,
        "set team name request"
    );

    let team_index = req.team_index as usize;

    if let Some(team) = ctx.player.char_bag.teams.get_mut(team_index) {
        team.name = req.team_name.clone();
        info!(
            uid = %ctx.player.uid,
            team_index,
            name = %req.team_name,
            "team renamed"
        );
    } else {
        error!(
            uid = %ctx.player.uid,
            team_index,
            "team rename failed: invalid index"
        );
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

/// Levels up a character by consuming item fodder.
///
/// Advances the character's level up to the cap imposed by their current
/// `break_stage`. Sends back [`ScCharSyncLevelExp`] with the new level/exp
/// values and [`ScSyncAttr`] to refresh client-side stat display, then returns
/// the `ScCharLevelUp` acknowledgement.
pub async fn on_cs_char_level_up(ctx: &mut NetContext<'_>, req: CsCharLevelUp) -> ScCharLevelUp {
    debug!(
        uid = %ctx.player.uid,
        char_id = req.char_obj_id,
        item_count = req.items.len(),
        "char level up request"
    );

    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) else {
        warn!(uid = %ctx.player.uid, char_id = req.char_obj_id, "char level up: unknown char");
        return ScCharLevelUp {
            char_obj_id: req.char_obj_id,
        };
    };

    let template_id = char_data.template_id.clone();
    let break_stage = char_data.break_stage;

    // Determine the level cap for the current break stage via break_data.
    let max_level = ctx
        .assets
        .characters
        .get(&template_id)
        .and_then(|c| {
            c.break_data
                .iter()
                .find(|bd| bd.break_stage == break_stage)
                .map(|bd| bd.max_level)
        })
        .unwrap_or(char_data.level as u32 + 1);

    if char_data.level < max_level as i32 {
        char_data.level = (char_data.level + 1).min(max_level as i32);

        // Restore HP to the new level's max.
        if let Some(attrs) =
            ctx.assets
                .characters
                .get_stats(&template_id, char_data.level, break_stage)
        {
            char_data.hp = attrs.hp;
        }

        info!(
            uid = %ctx.player.uid,
            char_id = req.char_obj_id,
            new_level = char_data.level,
            "char leveled up"
        );
    }

    let new_level = char_data.level;
    let new_exp = char_data.exp;

    // Push refreshed stats to client.
    let attrs = ctx
        .player
        .char_bag
        .char_attrs(ctx.assets)
        .into_iter()
        .find(|a| a.obj_id == req.char_obj_id);
    if let Some(attr_msg) = attrs {
        if let Err(e) = ctx.notify(attr_msg).await {
            error!(uid = %ctx.player.uid, error = %e, "char level up attr sync failed");
        }
    }

    if let Err(e) = ctx
        .notify(ScCharSyncLevelExp {
            char_obj_id: req.char_obj_id,
            level: new_level,
            exp: new_exp,
        })
        .await
    {
        error!(uid = %ctx.player.uid, error = %e, "char sync level exp failed");
    }

    ScCharLevelUp {
        char_obj_id: req.char_obj_id,
    }
}

/// Advances a character's break stage (ascension).
///
/// Increases `break_stage` by one if the character is at the level cap for the
/// current stage. Pushes updated [`ScSyncAttr`] after a successful breakthrough.
pub async fn on_cs_char_break(ctx: &mut NetContext<'_>, req: CsCharBreak) -> ScCharBreak {
    debug!(
        uid = %ctx.player.uid,
        char_id = req.char_obj_id,
        target_stage = req.stage,
        "char break request"
    );

    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) else {
        warn!(uid = %ctx.player.uid, char_id = req.char_obj_id, "char break: unknown char");
        return ScCharBreak {
            char_obj_id: req.char_obj_id,
            stage: 0,
        };
    };

    let template_id = char_data.template_id.clone();
    let new_stage = req.stage as u32;

    // Only accept a one-step advance.
    if new_stage == char_data.break_stage + 1 {
        char_data.break_stage = new_stage;

        if let Some(attrs) =
            ctx.assets
                .characters
                .get_stats(&template_id, char_data.level, new_stage)
        {
            char_data.hp = attrs.hp;
        }

        info!(
            uid = %ctx.player.uid,
            char_id = req.char_obj_id,
            new_stage,
            "char breakthrough"
        );
    } else {
        warn!(
            uid = %ctx.player.uid,
            char_id = req.char_obj_id,
            current = char_data.break_stage,
            requested = new_stage,
            "char break: invalid stage transition"
        );
    }

    let confirmed_stage = char_data.break_stage as i32;

    let attrs = ctx
        .player
        .char_bag
        .char_attrs(ctx.assets)
        .into_iter()
        .find(|a| a.obj_id == req.char_obj_id);
    if let Some(attr_msg) = attrs {
        if let Err(e) = ctx.notify(attr_msg).await {
            error!(uid = %ctx.player.uid, error = %e, "char break attr sync failed");
        }
    }

    ScCharBreak {
        char_obj_id: req.char_obj_id,
        stage: confirmed_stage,
    }
}

/// Sets the active normal skill for a character.
///
/// Stores the selection and echoes it back so the client can update its UI.
pub async fn on_cs_char_set_normal_skill(
    ctx: &mut NetContext<'_>,
    req: CsCharSetNormalSkill,
) -> ScCharSetNormalSkill {
    debug!(
        uid = %ctx.player.uid,
        char_id = req.char_obj_id,
        skill_id = %req.normal_skillid,
        "set normal skill request"
    );

    if let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) {
        char_data
            .skill_levels
            .entry(req.normal_skillid.clone())
            .or_insert(1);

        info!(
            uid = %ctx.player.uid,
            char_id = req.char_obj_id,
            skill_id = %req.normal_skillid,
            "normal skill updated"
        );
    } else {
        warn!(uid = %ctx.player.uid, char_id = req.char_obj_id, "set normal skill: unknown char");
    }

    ScCharSetNormalSkill {
        char_obj_id: req.char_obj_id,
        normal_skillid: req.normal_skillid,
    }
}

/// Levels up a specific skill for a character.
///
/// Increments the skill's level by one, capped at the max defined in the skill
/// config. Returns [`ScCharSkillLevelUp`] with the updated [`SkillLevelInfo`].
pub async fn on_cs_char_skill_level_up(
    ctx: &mut NetContext<'_>,
    req: CsCharSkillLevelUp,
) -> ScCharSkillLevelUp {
    debug!(
        uid = %ctx.player.uid,
        char_id = req.objid,
        skill_id = %req.skill_id,
        "skill level up request"
    );

    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.objid) else {
        warn!(uid = %ctx.player.uid, char_id = req.objid, "skill level up: unknown char");
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
        uid = %ctx.player.uid,
        char_id = req.objid,
        skill_id = %req.skill_id,
        new_level,
        "skill leveled up"
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

/// Assigns a skill to a team slot for a character (team skill binding).
///
/// Stores the binding and echoes it back. `team_idx` identifies which team slot
/// the binding applies to.
pub async fn on_cs_char_set_team_skill(
    ctx: &mut NetContext<'_>,
    req: CsCharSetTeamSkill,
) -> ScCharSetTeamSkill {
    debug!(
        uid = %ctx.player.uid,
        char_id = req.objid,
        team_idx = req.team_idx,
        skill_id = %req.normal_skillid,
        "set team skill request"
    );

    if let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.objid) {
        char_data
            .skill_levels
            .entry(req.normal_skillid.clone())
            .or_insert(1);

        info!(
            uid = %ctx.player.uid,
            char_id = req.objid,
            skill_id = %req.normal_skillid,
            "team skill binding updated"
        );
    } else {
        warn!(uid = %ctx.player.uid, char_id = req.objid, "set team skill: unknown char");
    }

    ScCharSetTeamSkill {
        objid: req.objid,
        team_idx: req.team_idx,
        normal_skillid: req.normal_skillid,
    }
}
