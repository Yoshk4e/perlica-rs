use crate::player::Player;
use anyhow::Result;
use perlica_proto::{
    BattleInfo, CharInfo, CharTeamInfo, CharTeamMemberInfo, ScSyncCharBagInfo, SkillInfo,
    SkillLevelInfo,
};

pub fn prepare_char_bag_sync(player: &Player) -> Result<ScSyncCharBagInfo> {
    let char_bag = &player.char_bag;
    let assets = player.resources;

    let team_states = char_bag.prepare_team_sync_states(assets);
    let char_states = char_bag.prepare_char_sync_states(assets)?;

    let team_info = team_states
        .into_iter()
        .map(|t| CharTeamInfo {
            team_name: t.name,
            char_team: t.char_ids,
            leaderid: t.leader_id,
            member_info: t
                .member_skills
                .into_iter()
                .map(|(id, skill)| {
                    (
                        id,
                        CharTeamMemberInfo {
                            normal_skillid: skill,
                        },
                    )
                })
                .collect(),
        })
        .collect();

    let char_info = char_states
        .into_iter()
        .map(|c| CharInfo {
            objid: c.objid,
            templateid: c.template_id,
            level: c.level,
            exp: c.exp,
            finish_break_stage: c.break_stage as i32,
            equip_col: Default::default(),
            equip_suit: Default::default(),
            normal_skill: c.normal_skill.clone(),
            is_dead: c.is_dead,
            weapon_id: c.weapon_id,
            own_time: c.own_time,
            battle_info: Some(BattleInfo {
                hp: c.hp,
                ultimatesp: c.ultimate_sp,
            }),
            skill_info: Some(SkillInfo {
                normal_skill: c.normal_skill,
                level_info: c
                    .skill_levels
                    .into_iter()
                    .map(|s| SkillLevelInfo {
                        skill_id: s.skill_id,
                        skill_level: s.skill_level,
                        skill_max_level: s.skill_max_level,
                    })
                    .collect(),
            }),
        })
        .collect();

    Ok(ScSyncCharBagInfo {
        char_info,
        team_info,
        curr_team_index: char_bag.meta.curr_team_index as i32,
        max_char_team_member_count: perlica_logic::character::char_bag::Team::SLOTS_COUNT as u32,
    })
}
