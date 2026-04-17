//! Character skill handlers: equip normal skill, level a skill up, set team skill.

use crate::net::NetContext;
use perlica_proto::{
    CsCharSetNormalSkill, CsCharSetTeamSkill, CsCharSkillLevelUp, ScCharSetNormalSkill,
    ScCharSetTeamSkill, ScCharSkillLevelUp, SkillLevelInfo,
};
use tracing::info;

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
