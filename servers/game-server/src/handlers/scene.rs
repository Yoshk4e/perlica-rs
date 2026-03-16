use crate::net::NetContext;
use perlica_logic::character::char_bag::CharIndex;
use perlica_proto::{
    BattleInfo, CsSceneKillChar, CsSceneKillMonster, CsSceneLoadFinish, CsSceneRevival,
    LeaveObjectInfo, ScCharSyncStatus, ScEnterSceneNotify, ScObjectEnterView, ScObjectLeaveView,
    ScSceneDestroyEntity, ScSceneRevival, ScSelfSceneInfo, SceneCharacter, SceneImplEmpty,
    SceneMonster, SceneObjectCommonInfo, SceneObjectDetailContainer, Vector,
    sc_self_scene_info::SceneImpl,
};
use tracing::{debug, error};

#[repr(i32)]
pub enum SelfInfoReason {
    EnterScene = 0,
    ReviveDead = 1,
    ReviveRest = 2,
    ChangeTeam = 3,
    ReviveByItem = 4,
    ResetDungeon = 5,
}

#[repr(i32)]
pub enum EntityDestroyReason {
    Immediately = 0,
    Dead = 1,
}

pub async fn notify_enter_scene(ctx: &mut NetContext<'_>) -> bool {
    let msg = ScEnterSceneNotify {
        role_id: 1,
        scene_name: ctx.player.world.last_scene.clone(),
        scene_id: ctx
            .assets
            .str_id_num
            .get_scene_id(&ctx.player.world.last_scene)
            .unwrap_or(0),
        position: Some(Vector {
            x: ctx.player.world.pos_x,
            y: ctx.player.world.pos_y,
            z: ctx.player.world.pos_z,
        }),
    };
    debug!(scene = %msg.scene_name, "enter scene");
    if let Err(e) = ctx.notify(msg).await {
        error!(error = %e, "enter scene failed");
        return false;
    }
    true
}

pub async fn notify_object_enter_view(
    ctx: &mut NetContext<'_>,
    scene_name: String,
    char_list: Vec<SceneCharacter>,
    monster_list: Vec<SceneMonster>,
) -> bool {
    let msg = ScObjectEnterView {
        scene_name: scene_name.clone(),
        scene_id: ctx.assets.str_id_num.get_scene_id(&scene_name).unwrap_or(0),
        detail: Some(SceneObjectDetailContainer {
            char_list,
            monster_list,
            ..Default::default()
        }),
        ..Default::default()
    };
    if let Err(e) = ctx.notify(msg).await {
        error!(error = %e, "object enter view failed");
        return false;
    }
    true
}

pub async fn on_scene_load_finish(
    ctx: &mut NetContext<'_>,
    req: CsSceneLoadFinish,
) -> ScSelfSceneInfo {
    ctx.player.world.last_scene = req.scene_name.clone();

    let char_list = pack_scene_chars(ctx);
    let monster_list = pack_scene_monsters(ctx, &req.scene_name);

    if !notify_object_enter_view(ctx, req.scene_name.clone(), char_list.clone(), monster_list.clone()).await
    {
        error!("object enter view failed");
    }
    if !post_load_sync(ctx).await {
        error!("post-load sync failed");
    }

    ScSelfSceneInfo {
        scene_name: req.scene_name.clone(),
        scene_id: ctx
            .assets
            .str_id_num
            .get_scene_id(&req.scene_name)
            .unwrap_or(0),
        self_info_reason: SelfInfoReason::EnterScene as i32,
        scene_impl: Some(SceneImpl::Empty(SceneImplEmpty {})),
        detail: Some(SceneObjectDetailContainer {
            char_list,
			monster_list,
            ..Default::default()
        }),
        level_scripts: vec![],
        ..Default::default()
    }
}

fn pack_scene_chars(ctx: &NetContext<'_>) -> Vec<SceneCharacter> {
    let char_bag = &ctx.player.char_bag;
    let team = &char_bag.teams[char_bag.meta.curr_team_index as usize];

    let spawn_pos = Vector {
        x: ctx.player.world.pos_x,
        y: ctx.player.world.pos_y,
        z: ctx.player.world.pos_z,
    };
    let spawn_rot = Vector {
        x: ctx.player.world.rot_x,
        y: ctx.player.world.rot_y,
        z: ctx.player.world.rot_z,
    };

    let chars = team
        .char_team
        .iter()
        .filter_map(|slot| slot.char_index())
        .map(|idx| {
            let char_data = &char_bag.chars[idx.as_usize()];
            SceneCharacter {
                common_info: Some(SceneObjectCommonInfo {
                    id: idx.object_id(),
                    templateid: char_data.template_id.clone(),
                    position: Some(spawn_pos.clone()),
                    rotation: Some(spawn_rot.clone()),
                    belong_level_script_id: 0,
                    r#type: 0,
                }),
                level: 15,
                name: "Yoshk4e".to_string(),
            }
        })
        .collect::<Vec<_>>();

    debug!(count = chars.len(), "scene chars packed");
    chars
}
//Monster level is set to 5 to downscale them to character power level. At least until character leveling is a thing
fn pack_scene_monsters(ctx: &NetContext<'_>, scene_name: &str) -> Vec<SceneMonster> {
    let Some(spawns) = ctx.assets.enemy_spawns.get(scene_name) else {
        return vec![];
    };
    let monsters = spawns
        .iter()
        .enumerate()
        .map(|(i, enemy)| SceneMonster {
            common_info: Some(SceneObjectCommonInfo {
                id: 1000 + i as u64,
                templateid: enemy.template_id.clone(),
                position: Some(Vector {
                    x: enemy.position.x,
                    y: enemy.position.y,
                    z: enemy.position.z,
                }),
                rotation: Some(Vector {
                    x: enemy.rotation.x,
                    y: enemy.rotation.y,
                    z: enemy.rotation.z,
                }),
                belong_level_script_id: 0,
                r#type: 16,
            }),
            origin_id: 0,
            level: 5,
			//level: enemy.level as i32,
        })
        .collect::<Vec<_>>();

    debug!(count = monsters.len(), "scene monsters packed");
    monsters
}

pub async fn post_load_sync(ctx: &mut NetContext<'_>) -> bool {
    let ok_attrs = super::char_bag::push_char_attrs(ctx).await;
    let ok_status = super::char_bag::push_char_status(ctx).await;
    ok_attrs && ok_status
}

pub async fn on_cs_scene_kill_monster(ctx: &mut NetContext<'_>, req: CsSceneKillMonster) {
    let _ = ctx
        .notify(ScSceneDestroyEntity {
            scene_name: ctx.player.world.last_scene.clone(),
            id: req.id,
            reason: EntityDestroyReason::Dead as i32,
        })
        .await;
}

pub async fn on_cs_scene_kill_char(ctx: &mut NetContext<'_>, req: CsSceneKillChar) {
    if let Some(char) = ctx.player.char_bag.get_char_by_objid_mut(req.id) {
        char.is_dead = true;
    }
    let _ = ctx
        .notify(ScSceneDestroyEntity {
            scene_name: ctx.player.world.last_scene.clone(),
            id: req.id,
            reason: EntityDestroyReason::Dead as i32,
        })
        .await;
}

pub async fn on_cs_scene_revival(ctx: &mut NetContext<'_>, _req: CsSceneRevival) -> ScObjectEnterView {
    let revive_chars: Vec<u64> = ctx
        .player
        .char_bag
        .chars
        .iter()
        .enumerate()
        .filter(|(_, c)| c.is_dead)
        .map(|(i, _)| CharIndex::from_usize(i).object_id())
        .collect();

    for &objid in &revive_chars {
        if let Some(char) = ctx.player.char_bag.get_char_by_objid_mut(objid) {
            char.is_dead = false;
            char.hp = ctx
                .assets
                .characters
                .get_stats(&char.template_id.clone(), char.level, char.break_stage)
                .map(|a| a.hp / 2.0)
                .unwrap_or(50.0);
        }
    }

    for &objid in &revive_chars {
        if let Some(char) = ctx.player.char_bag.get_char_by_objid_mut(objid) {
            let hp = char.hp;
            let sp = char.ultimate_sp;
            drop(char);
            let _ = ctx
                .notify(ScCharSyncStatus {
                    objid,
                    is_dead: false,
                    battle_info: Some(BattleInfo { hp, ultimatesp: sp }),
                })
                .await;
        }
    }

    let scene_name = ctx.player.world.last_scene.clone();
    let scene_id = ctx.assets.str_id_num.get_scene_id(&scene_name).unwrap_or(0);
    let char_list = pack_scene_chars(ctx);
    let monster_list = pack_scene_monsters(ctx, &scene_name);
    let _ = ctx
        .notify(ScSelfSceneInfo {
            scene_name: scene_name.clone(),
            scene_id: scene_id.clone(),
            self_info_reason: SelfInfoReason::ReviveDead as i32,
            revive_chars,
            scene_impl: Some(SceneImpl::Empty(SceneImplEmpty {})),
            detail: Some(SceneObjectDetailContainer {
                char_list: char_list.clone(),
                monster_list: monster_list.clone(),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await;
		
	let _ = ctx
		.notify(ScSceneRevival {})
		.await;
		
	ScObjectEnterView {
        scene_name: scene_name.clone(),
        scene_id: scene_id.clone(),
        detail: Some(SceneObjectDetailContainer {
            char_list: char_list.clone(),
            monster_list: monster_list.clone(),
            ..Default::default()
        }),
        ..Default::default()
	}
		
}
