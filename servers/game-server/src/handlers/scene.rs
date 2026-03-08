use crate::net::NetContext;
use perlica_proto::{
    CsSceneKillChar, CsSceneLoadFinish, LeaveObjectInfo, ScEnterSceneNotify, ScObjectEnterView,
    ScObjectLeaveView, ScSelfSceneInfo, SceneCharacter, SceneMonster, SceneImplEmpty, SceneObjectCommonInfo,
    SceneObjectDetailContainer, Vector, sc_self_scene_info::SceneImpl,
};
use tracing::{debug, error, instrument};

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

#[instrument(skip(ctx, char_list, monster_list), fields(uid = %ctx.player.uid, scene = %scene_name, chars = char_list.len()))]
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

#[instrument(skip(ctx), fields(uid = %ctx.player.uid, scene = %req.scene_name))]
pub async fn on_scene_load_finish(
    ctx: &mut NetContext<'_>,
    req: CsSceneLoadFinish,
) -> ScSelfSceneInfo {
    ctx.player.world.last_scene = req.scene_name.clone();

    let char_list = pack_scene_chars(ctx);
	let monster_list = pack_scene_monsters(ctx, req.scene_name.clone());

    if !notify_object_enter_view(ctx, req.scene_name.clone(), char_list.clone(), monster_list.clone()).await {
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
        self_info_reason: 1,
        scene_impl: Some(SceneImpl::Empty(SceneImplEmpty {})),
        detail: Some(SceneObjectDetailContainer {
            char_list,
            ..Default::default()
        }),
        level_scripts: vec![],
        ..Default::default()
    }
}

fn pack_scene_monsters(ctx: &NetContext<'_>, scene_name: String) -> Vec<SceneMonster> {
	let enemy_spawns = &ctx.assets.enemy_spawns;

	let scene_enemy_data = enemy_spawns.get(&scene_name);
	
	let mut ind: u64 = 0;
	
	let mut monsters: Vec<SceneMonster> = Vec::new();
	
	if let Some(data) = scene_enemy_data {
		for enemy in data{
			let en = SceneMonster {
					common_info: Some(SceneObjectCommonInfo {
						id: ind + 100,
						templateid: enemy.template_id.to_string(),
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
				};
		ind += 1;
		monsters.push(en);
		};
	};

    debug!(count = monsters.len(), "scene monsters packed");
    monsters
}

fn pack_scene_chars(ctx: &NetContext<'_>) -> Vec<SceneCharacter> {
    let char_bag = &ctx.player.char_bag;
    let team = &char_bag.teams[char_bag.meta.curr_team_index as usize];

    // Use last known position from world state.
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

    let chars: Vec<SceneCharacter> = team
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
        .collect();

    debug!(count = chars.len(), "scene chars packed");
    chars
}

pub async fn on_cs_scene_kill_char(
    ctx: &mut NetContext<'_>,
    req: CsSceneKillChar,
) -> ScObjectLeaveView {
    ScObjectLeaveView {
        scene_name: ctx.player.world.last_scene.clone(),
        scene_id: ctx
            .assets
            .str_id_num
            .get_scene_id(&ctx.player.world.last_scene)
            .unwrap_or(0),
        obj_list: vec![LeaveObjectInfo {
            obj_type: 0,
            obj_id: req.id,
        }],
    }
}

#[instrument(skip(ctx), fields(uid = %ctx.player.uid))]
pub async fn post_load_sync(ctx: &mut NetContext<'_>) -> bool {
    let ok_attrs = super::char_bag::push_char_attrs(ctx).await;
    let ok_status = super::char_bag::push_char_status(ctx).await;
    ok_attrs && ok_status
}
