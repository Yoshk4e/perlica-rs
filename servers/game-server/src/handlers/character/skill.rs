//! Character skill handlers: equip normal skill, level a skill up, set team skill.

use crate::net::NetContext;
use perlica_logic::character::skill::max_skill_level;
use perlica_proto::{
    CsCharSetNormalSkill, CsCharSetTeamSkill, CsCharSkillLevelUp, ScCharSetNormalSkill,
    ScCharSetTeamSkill, ScCharSkillLevelUp, SkillLevelInfo,
};
use tracing::{info, warn};

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
    if let Err(e) = ctx
        .db
        .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
        .await
    {
        warn!(
            "Failed to persist char_bag after set normal skill: uid={}, error={}",
            ctx.player.uid, e
        );
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
    let max_level = max_skill_level(&template_id, &req.skill_id, ctx.assets);
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
    if let Err(e) = ctx
        .db
        .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
        .await
    {
        warn!(
            "Failed to persist char_bag after skill level up: uid={}, error={}",
            ctx.player.uid, e
        );
    }

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
    if let Err(e) = ctx
        .db
        .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
        .await
    {
        warn!(
            "Failed to persist char_bag after set team skill: uid={}, error={}",
            ctx.player.uid, e
        );
    }

    ScCharSetTeamSkill {
        objid: req.objid,
        team_idx: req.team_idx,
        normal_skillid: req.normal_skillid,
    }
}
