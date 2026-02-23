use crate::session::NetContext;
use anyhow::{Context, Result};
use config::BeyondAssets;
use perlica_logic::character::char_bag::{CharBag, CharIndex, Team};
use perlica_proto::sc_self_scene_info::SceneImpl;
use perlica_proto::{
    BattleInfo, CharInfo, CharTeamInfo, CharTeamMemberInfo, CsCharSetBattleInfo, CsLogin,
    CsMoveObjectMove, CsPing, CsSceneLoadFinish, ScCharSyncStatus, ScEnterSceneNotify, ScLogin,
    ScMoveObjectMove, ScObjectEnterView, ScPing, ScSelfSceneInfo, ScSyncCharBagInfo,
    SceneCharacter, SceneImplEmpty, SceneObjectCommonInfo, SceneObjectDetailContainer, SkillInfo,
    SkillLevelInfo, Vector,
};
use tracing::debug;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum LoadingState {
    Login,
    ScLogin,
    CharBagSync,
    EnterScene,
    Complete,
}

pub struct Player {
    pub uid: String,
    pub loading_state: LoadingState,
    pub char_bag: CharBag,
    pub resources: &'static BeyondAssets,
}

impl Player {
    pub fn new(resources: &'static BeyondAssets, uid: String) -> Self {
        Self {
            uid: uid.to_string(),
            loading_state: LoadingState::Login,
            char_bag: CharBag::new_with_starter(resources, &uid).unwrap_or_else(|_| CharBag::new()),
            resources,
        }
    }

    pub fn on_login(&mut self, uid: String) {
        self.uid = uid;
        self.loading_state = LoadingState::ScLogin;
        debug!("Player logged in, state now ScLogin");
    }

    pub fn advance_state(&mut self) {
        let old = self.loading_state;
        self.loading_state = match self.loading_state {
            LoadingState::Login => LoadingState::ScLogin,
            LoadingState::ScLogin => LoadingState::CharBagSync,
            LoadingState::CharBagSync => LoadingState::EnterScene,
            LoadingState::EnterScene => LoadingState::Complete,
            LoadingState::Complete => LoadingState::Complete,
        };
        debug!(
            "Loading state changed from {:?} to {:?}",
            old, self.loading_state
        );
    }
}

pub async fn on_login(ctx: &mut NetContext<'_>, req: CsLogin) -> ScLogin {
    ctx.player.on_login(req.uid.clone());
    debug!("Sending ScLogin for UID {}", req.uid);

    ScLogin {
        uid: req.uid,
        is_first_login: false,
        server_public_key: vec![],
        server_encryp_nonce: vec![],
        last_recv_up_seqid: ctx.client_seq_id,
        is_reconnect: false,
        is_enc: false,
        is_client_reconnect: false,
    }
}

pub async fn on_csping(ctx: &mut NetContext<'_>, req: CsPing) -> ScPing {
    if ctx.player.loading_state == LoadingState::ScLogin {
        ctx.player.advance_state();

        match sc_sync_char_bag(ctx).await {
            Ok(sync_msg) => {
                debug!(
                    "Sending ScSyncCharBagInfo with {} characters and {} teams",
                    sync_msg.char_info.len(),
                    sync_msg.team_info.len()
                );
                let _ = ctx.notify(sync_msg).await;

                let enter_msg = enter_scene(ctx).await;
                debug!(
                    "Sending ScEnterSceneNotify for scene {}",
                    enter_msg.scene_name
                );
                let _ = ctx.notify(enter_msg).await;

                ctx.player.advance_state();
            }
            Err(e) => tracing::error!("char bag sync failed: {}", e),
        }
    }

    ScPing {
        client_ts: req.client_ts,
        server_ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    }
}

pub async fn sc_sync_char_bag(ctx: &mut NetContext<'_>) -> Result<ScSyncCharBagInfo> {
    let char_bag = &ctx.player.char_bag;
    let assets = &ctx.resources;

    let mut sync_msg = ScSyncCharBagInfo {
        char_info: Vec::new(),
        team_info: Vec::new(),
        curr_team_index: char_bag.meta.curr_team_index as i32,
        max_char_team_member_count: Team::SLOTS_COUNT as u32,
    };

    for team in &char_bag.teams {
        let char_team: Vec<u64> = team
            .char_team
            .iter()
            .filter_map(|slot| slot.char_index())
            .map(|idx| idx.object_id())
            .collect();

        let member_info = team
            .char_team
            .iter()
            .filter_map(|slot| slot.char_index())
            .map(|idx| {
                let char_data = &char_bag.chars[idx.as_usize()];
                let normal_skill = assets
                    .char_skills
                    .get_char_skills(&char_data.template_id)
                    .into_iter()
                    .find_map(|b| {
                        b.entries
                            .first()
                            .filter(|e| e.skill_id.contains("normal_skill"))
                            .map(|e| e.skill_id.clone())
                    })
                    .unwrap_or_default();
                (
                    idx.object_id(),
                    CharTeamMemberInfo {
                        normal_skillid: normal_skill,
                    },
                )
            })
            .collect();

        sync_msg.team_info.push(CharTeamInfo {
            team_name: team.name.clone(),
            char_team,
            leaderid: team.leader_index.object_id(),
            member_info,
        });
    }

    for (i, char) in char_bag.chars.iter().enumerate() {
        let index = CharIndex::from_usize(i);
        let char_template = assets
            .characters
            .get(&char.template_id)
            .with_context(|| format!("Unknown character template: {}", char.template_id))?;

        let skill_bundles = assets.char_skills.get_char_skills(&char_template.char_id);
        let normal_skill = skill_bundles
            .iter()
            .find_map(|b| {
                b.entries
                    .first()
                    .filter(|e| e.skill_id.contains("normal_skill"))
                    .map(|e| e.skill_id.clone())
            })
            .unwrap_or_default();

        let mut level_info = Vec::new();
        for bundle in &skill_bundles {
            let current_level = char
                .skill_levels
                .get(&bundle.entries[0].skill_id)
                .copied()
                .unwrap_or(1);
            if let Some(entry) = bundle.entries.iter().find(|e| e.level == current_level) {
                level_info.push(SkillLevelInfo {
                    skill_id: entry.skill_id.clone(),
                    skill_level: entry.level as i32,
                    skill_max_level: bundle.entries.iter().map(|e| e.level).max().unwrap_or(1)
                        as i32,
                });
            }
        }

        let char_info = CharInfo {
            objid: index.object_id(),
            templateid: char.template_id.clone(),
            level: char.level,
            exp: char.exp,
            finish_break_stage: char.break_stage as i32,
            equip_col: ::std::collections::HashMap::new(),
            normal_skill: normal_skill.clone(),
            is_dead: char.is_dead,
            weapon_id: char.weapon_id.inst_id(),
            own_time: char.own_time,
            equip_suit: ::std::collections::HashMap::new(),
            battle_info: Some(BattleInfo {
                hp: char.hp,
                ultimatesp: char.ultimate_sp,
            }),
            skill_info: Some(SkillInfo {
                level_info,
                normal_skill,
            }),
        };
        sync_msg.char_info.push(char_info);
    }

    debug!(
        "ScSyncCharBagInfo ready with {} characters",
        sync_msg.char_info.len()
    );
    Ok(sync_msg)
}

pub async fn enter_scene(ctx: &mut NetContext<'_>) -> ScEnterSceneNotify {
    let msg = ScEnterSceneNotify {
        role_id: 1,
        scene_name: "map01_dg003".to_string(),
        scene_id: 11,
        position: Some(Vector {
            x: 823.0,
            y: -30.0,
            z: 69.0,
        }),
    };
    debug!("Built ScEnterSceneNotify for scene {}", msg.scene_name);
    msg
}

pub async fn send_sc_object_enter_view(
    ctx: &mut NetContext<'_>,
    scene_name: String,
    char_list: Vec<SceneCharacter>,
) {
    let enter_view = ScObjectEnterView {
        scene_name: scene_name.clone(),
        detail: Some(SceneObjectDetailContainer {
            char_list,
            interactive_list: vec![],
            monster_list: vec![],
            ..Default::default()
        }),
        ..Default::default()
    };

    debug!(
        "Sending ScObjectEnterView for scene {} with {} characters",
        scene_name,
        enter_view
            .detail
            .as_ref()
            .map(|d| d.char_list.len())
            .unwrap_or(0)
    );
    let _ = ctx.notify(enter_view).await;
}

pub async fn on_scene_load_finish(
    ctx: &mut NetContext<'_>,
    req: CsSceneLoadFinish,
) -> ScSelfSceneInfo {
    debug!("Handling CsSceneLoadFinish for scene {}", req.scene_name);

    let char_bag = &ctx.player.char_bag;
    let team = &char_bag.teams[char_bag.meta.curr_team_index as usize];

    let init_pos = Vector {
        x: 227.9,
        y: 137.6,
        z: 297.0,
    };
    let init_rot = Vector {
        x: 0.0,
        y: 90.0,
        z: 0.0,
    };

    let char_list: Vec<SceneCharacter> = team
        .char_team
        .iter()
        .filter_map(|slot| slot.char_index())
        .map(|char_idx| {
            let char_data = &char_bag.chars[char_idx.as_usize()];
            SceneCharacter {
                common_info: Some(SceneObjectCommonInfo {
                    id: char_idx.object_id(),
                    templateid: char_data.template_id.clone(),
                    position: Some(init_pos.clone()),
                    rotation: Some(init_rot.clone()),
                    belong_level_script_id: 0,
                    r#type: 0,
                }),
                level: 15,
                name: "Yoshk4e".to_string(),
            }
        })
        .collect();

    debug!("Built {} player characters for spawn", char_list.len());

    send_sc_object_enter_view(ctx, req.scene_name.clone(), char_list.clone()).await;

    let self_info = ScSelfSceneInfo {
        scene_name: req.scene_name,
        scene_id: 11,
        self_info_reason: 1,
        scene_impl: Some(SceneImpl::Empty(SceneImplEmpty {})),
        detail: Some(SceneObjectDetailContainer {
            char_list,
            ..Default::default()
        }),
        level_scripts: vec![],
        ..Default::default()
    };

    debug!("Sending ScSelfSceneInfo");
    self_info
}

pub async fn on_cs_char_set_battle_info(
    ctx: &mut NetContext<'_>,
    req: CsCharSetBattleInfo,
) -> ScCharSyncStatus {
    debug!("Received CsCharSetBattleInfo for objid {}", req.objid);

    if let (Some(char), Some(battle_info)) = (
        ctx.player.char_bag.get_char_by_objid_mut(req.objid),
        req.battle_info.clone(),
    ) {
        char.hp = battle_info.hp;
        char.ultimate_sp = battle_info.ultimatesp;
        debug!(
            "Updated char {} HP={} UltimateSP={}",
            req.objid, battle_info.hp, battle_info.ultimatesp
        );
    } else {
        debug!("Char or battle_info missing for objid {}", req.objid);
    }

    ScCharSyncStatus {
        objid: req.objid,
        is_dead: req
            .battle_info
            .as_ref()
            .map(|b| b.hp <= 0.0)
            .unwrap_or(false),
        battle_info: req.battle_info,
    }
}

pub async fn on_cs_move_object_move(
    ctx: &mut NetContext<'_>,
    req: CsMoveObjectMove,
) -> ScMoveObjectMove {
    // Log every movement packet (very useful for debugging)
    for info in &req.move_info {
        debug!(
            "Move object {} | pos: {:?} | rot: {:?} | speed: {:?} | state: {}",
            info.objid,
            info.motion_info.as_ref().and_then(|m| m.position.as_ref()),
            info.motion_info.as_ref().and_then(|m| m.rotation.as_ref()),
            info.motion_info.as_ref().and_then(|m| m.speed.as_ref()),
            info.motion_info.as_ref().map(|m| m.state).unwrap_or(0)
        );
    }

    // Echo the exact same movement data back (standard for these alphas)
    // Client expects the server to confirm/relay the move
    ScMoveObjectMove {
        move_info: req.move_info, // mirror everything the client sent
        server_notify: true,      // tells client this is authoritative server response
    }
}
