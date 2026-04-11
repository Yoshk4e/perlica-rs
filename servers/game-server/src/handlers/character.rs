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
        "Battle info update: objid={}, has_battle_info={}",
        req.objid,
        req.battle_info.is_some()
    );

    if let Some(bi) = &req.battle_info {
        ctx.player
            .char_bag
            .update_battle_info(req.objid, bi.hp, bi.ultimatesp);

        debug!(
            "Battle info updated: objid={}, hp={}, ultimate_sp={}",
            req.objid, bi.hp, bi.ultimatesp
        );
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
        let leader_in_team = team.char_team.iter().any(|slot| {
            slot.char_index()
                .map(|idx| idx.object_id() == req.leaderid)
                .unwrap_or(false)
        });

        if leader_in_team {
            team.leader_index = CharIndex::from_object_id(req.leaderid);
            info!(
                "Team leader updated: team_index={}, leader_id={}",
                req.team_index, req.leaderid
            );
        } else {
            warn!(
                "Rejected team leader update: leader_id={} is not in team_index={}",
                req.leaderid, req.team_index
            );
        }
    } else {
        error!(
            "Invalid team index for team leader update: team_index={}",
            req.team_index
        );
    }

    ScCharBagSetTeamLeader {
        team_index: req.team_index,
        leaderid: req.leaderid,
    }
}

/// Switches active team. ACK is sent first so the client processes the index change before the scene diff.
pub async fn on_cs_char_bag_set_curr_team_index(
    ctx: &mut NetContext<'_>,
    req: CsCharBagSetCurrTeamIndex,
) {
    debug!(
        "Set current team index request: new_team_index={}",
        req.team_index
    );

    let old_team_index = ctx.player.char_bag.meta.curr_team_index as usize;
    let new_team_index = req.team_index as usize;

    if new_team_index >= ctx.player.char_bag.teams.len() {
        error!(
            "Invalid team index: team_index={}, max_teams={}",
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
        debug!(
            "Current team index unchanged: team_index={}",
            req.team_index
        );

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
        "Team index changed: old={}, new={}, old_count={}, new_count={}",
        old_team_index,
        new_team_index,
        old_team_ids.len(),
        new_team_ids.len()
    );

    if let Err(error) = ctx
        .send(ScCharBagSetCurrTeamIndex {
            team_index: req.team_index,
        })
        .await
    {
        error!(
            "Failed to send current team index acknowledgement: {:?}",
            error
        );
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
        if let Err(error) = ctx.notify(leave).await {
            error!("Failed to notify team leave view: {:?}", error);
        }
    }

    if let Err(error) = ctx.notify(enter_view).await {
        error!("Failed to notify team enter view: {:?}", error);
    }

    if let Err(error) = ctx.notify(self_info).await {
        error!("Failed to notify team self info: {:?}", error);
    }

    super::char_bag::push_char_status_for_ids(ctx, &new_team_ids).await;
}

/// Sets team composition. ACK goes first so the client sees the new roster before the scene diff.
pub async fn on_cs_char_bag_set_team(ctx: &mut NetContext<'_>, req: CsCharBagSetTeam) {
    let uid = ctx.player.uid.clone();
    let team_index = req.team_index as usize;

    debug!(
        "Received character bag set team request: uid={}, team_index={}, char_count={}",
        uid,
        req.team_index,
        req.char_team.len()
    );

    if team_index >= ctx.player.char_bag.teams.len() {
        warn!(
            "Rejected character bag set team request: uid={}, invalid team_index={}, max_teams={}",
            uid,
            req.team_index,
            ctx.player.char_bag.teams.len()
        );

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

    if let Err(error) = ctx
        .send(ScCharBagSetTeam {
            team_index: req.team_index,
            char_team: req.char_team.clone(),
        })
        .await
    {
        error!(
            "Failed to send character bag set team acknowledgement: uid={}, team_index={}, error={:?}",
            uid, req.team_index, error
        );
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
            if let Err(error) = ctx.notify(leave).await {
                error!(
                    "Failed to notify team leave view: uid={}, error={:?}",
                    uid, error
                );
            }
        }

        if let Err(error) = ctx.notify(enter_view).await {
            error!(
                "Failed to notify team enter view: uid={}, error={:?}",
                uid, error
            );
        }

        if let Err(error) = ctx.notify(self_info).await {
            error!(
                "Failed to notify team self info: uid={}, error={:?}",
                uid, error
            );
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

        if let Err(error) = ctx.notify(self_info).await {
            error!(
                "Failed to notify inactive team update: uid={}, error={:?}",
                uid, error
            );
        }
    }
}

/// Renames a team. Echoes an empty string if the team index is out of range.
pub async fn on_cs_char_bag_set_team_name(
    ctx: &mut NetContext<'_>,
    req: CsCharBagSetTeamName,
) -> ScCharBagSetTeamName {
    debug!(
        "Set team name request: uid={}, team_index={}, name={}",
        ctx.player.uid, req.team_index, req.team_name
    );

    let team_index = req.team_index as usize;

    if let Some(team) = ctx.player.char_bag.teams.get_mut(team_index) {
        team.name = req.team_name.clone();
        info!(
            "Team renamed: uid={}, team_index={}, name={}",
            ctx.player.uid, team_index, req.team_name
        );
    } else {
        error!(
            "Team rename failed: uid={}, invalid team_index={}",
            ctx.player.uid, team_index
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

/// Levels up a character. Level is capped by `break_stage`. Pushes updated attrs before the ACK.
pub async fn on_cs_char_level_up(ctx: &mut NetContext<'_>, req: CsCharLevelUp) -> ScCharLevelUp {
    debug!(
        "Character level up request: uid={}, char_id={}, item_count={}",
        ctx.player.uid,
        req.char_obj_id,
        req.items.len()
    );

    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) else {
        warn!(
            "Character level up failed: uid={}, unknown char_id={}",
            ctx.player.uid, req.char_obj_id
        );
        return ScCharLevelUp {
            char_obj_id: req.char_obj_id,
        };
    };

    let template_id = char_data.template_id.clone();
    let break_stage = char_data.break_stage;

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

        if let Some(attrs) =
            ctx.assets
                .characters
                .get_stats(&template_id, char_data.level, break_stage)
        {
            char_data.hp = attrs.hp;
        }

        info!(
            "Character leveled up: uid={}, char_id={}, new_level={}",
            ctx.player.uid, req.char_obj_id, char_data.level
        );
    }

    let new_level = char_data.level;
    let new_exp = char_data.exp;

    let attrs = ctx
        .player
        .char_bag
        .char_attrs(ctx.assets)
        .into_iter()
        .find(|a| a.obj_id == req.char_obj_id);

    if let Some(attr_msg) = attrs {
        if let Err(error) = ctx.notify(attr_msg).await {
            error!(
                "Failed to sync character attributes after level up: uid={}, error={:?}",
                ctx.player.uid, error
            );
        }
    }

    if let Err(error) = ctx
        .notify(ScCharSyncLevelExp {
            char_obj_id: req.char_obj_id,
            level: new_level,
            exp: new_exp,
        })
        .await
    {
        error!(
            "Failed to sync character level and exp: uid={}, error={:?}",
            ctx.player.uid, error
        );
    }

    ScCharLevelUp {
        char_obj_id: req.char_obj_id,
    }
}

/// Advances break stage by one if the character is at the current level cap.
pub async fn on_cs_char_break(ctx: &mut NetContext<'_>, req: CsCharBreak) -> ScCharBreak {
    debug!(
        "Character break request: uid={}, char_id={}, target_stage={}",
        ctx.player.uid, req.char_obj_id, req.stage
    );

    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) else {
        warn!(
            "Character break failed: uid={}, unknown char_id={}",
            ctx.player.uid, req.char_obj_id
        );
        return ScCharBreak {
            char_obj_id: req.char_obj_id,
            stage: 0,
        };
    };

    let template_id = char_data.template_id.clone();
    let new_stage = req.stage as u32;

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
            "Character breakthrough complete: uid={}, char_id={}, new_stage={}",
            ctx.player.uid, req.char_obj_id, new_stage
        );
    } else {
        warn!(
            "Character break rejected: uid={}, char_id={}, current_stage={}, requested_stage={}",
            ctx.player.uid, req.char_obj_id, char_data.break_stage, new_stage
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
        if let Err(error) = ctx.notify(attr_msg).await {
            error!(
                "Failed to sync character attributes after breakthrough: uid={}, error={:?}",
                ctx.player.uid, error
            );
        }
    }

    ScCharBreak {
        char_obj_id: req.char_obj_id,
        stage: confirmed_stage,
    }
}

/// Sets the active normal skill and echoes the selection back.
pub async fn on_cs_char_set_normal_skill(
    ctx: &mut NetContext<'_>,
    req: CsCharSetNormalSkill,
) -> ScCharSetNormalSkill {
    debug!(
        "Set normal skill request: uid={}, char_id={}, skill_id={}",
        ctx.player.uid, req.char_obj_id, req.normal_skillid
    );

    if let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) {
        char_data
            .skill_levels
            .entry(req.normal_skillid.clone())
            .or_insert(1);

        info!(
            "Normal skill updated: uid={}, char_id={}, skill_id={}",
            ctx.player.uid, req.char_obj_id, req.normal_skillid
        );
    } else {
        warn!(
            "Set normal skill ignored: uid={}, unknown char_id={}",
            ctx.player.uid, req.char_obj_id
        );
    }

    ScCharSetNormalSkill {
        char_obj_id: req.char_obj_id,
        normal_skillid: req.normal_skillid,
    }
}

/// Increments a skill's level by one, capped at the config max.
pub async fn on_cs_char_skill_level_up(
    ctx: &mut NetContext<'_>,
    req: CsCharSkillLevelUp,
) -> ScCharSkillLevelUp {
    debug!(
        "Skill level up request: uid={}, char_id={}, skill_id={}",
        ctx.player.uid, req.objid, req.skill_id
    );

    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.objid) else {
        warn!(
            "Skill level up failed: uid={}, unknown char_id={}",
            ctx.player.uid, req.objid
        );
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
        "Skill leveled up: uid={}, char_id={}, skill_id={}, new_level={}",
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

/// Binds a skill to a team slot and echoes the binding back.
pub async fn on_cs_char_set_team_skill(
    ctx: &mut NetContext<'_>,
    req: CsCharSetTeamSkill,
) -> ScCharSetTeamSkill {
    debug!(
        "Set team skill request: uid={}, char_id={}, team_idx={}, skill_id={}",
        ctx.player.uid, req.objid, req.team_idx, req.normal_skillid
    );

    if let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.objid) {
        char_data
            .skill_levels
            .entry(req.normal_skillid.clone())
            .or_insert(1);

        info!(
            "Team skill binding updated: uid={}, char_id={}, skill_id={}",
            ctx.player.uid, req.objid, req.normal_skillid
        );
    } else {
        warn!(
            "Set team skill ignored: uid={}, unknown char_id={}",
            ctx.player.uid, req.objid
        );
    }

    ScCharSetTeamSkill {
        objid: req.objid,
        team_idx: req.team_idx,
        normal_skillid: req.normal_skillid,
    }
}
