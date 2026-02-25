use crate::net::NetContext;
use perlica_proto::{
    CsSceneLoadFinish, ScEnterSceneNotify, ScObjectEnterView, ScSelfSceneInfo, SceneCharacter,
    SceneImplEmpty, SceneObjectCommonInfo, SceneObjectDetailContainer, Vector,
    sc_self_scene_info::SceneImpl,
};
use tracing::{debug, error, instrument};

#[instrument(skip(ctx), fields(uid = %ctx.player.uid))]
pub async fn notify_enter_scene(ctx: &mut NetContext<'_>) -> bool {
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
    debug!(scene = %msg.scene_name, "enter scene");
    if let Err(e) = ctx.notify(msg).await {
        error!(error = %e, "enter scene failed");
        return false;
    }
    true
}

#[instrument(skip(ctx, char_list), fields(uid = %ctx.player.uid, scene = %scene_name, chars = char_list.len()))]
pub async fn notify_object_enter_view(
    ctx: &mut NetContext<'_>,
    scene_name: String,
    char_list: Vec<SceneCharacter>,
) -> bool {
    let msg = ScObjectEnterView {
        scene_name: scene_name.clone(),
        scene_id: 11,
        detail: Some(SceneObjectDetailContainer {
            char_list,
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
    let char_list = pack_scene_chars(ctx);

    if !notify_object_enter_view(ctx, req.scene_name.clone(), char_list.clone()).await {
        error!("object enter view failed");
    }
    if !post_load_sync(ctx).await {
        error!("post-load sync failed");
    }

    ScSelfSceneInfo {
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
    }
}

// Packs the active team into the scene character list using a fixed spawn position.
fn pack_scene_chars(ctx: &NetContext<'_>) -> Vec<SceneCharacter> {
    let char_bag = &ctx.player.char_bag;
    let team = &char_bag.teams[char_bag.meta.curr_team_index as usize];

    let spawn_pos = Vector { x: 227.9, y: 137.6, z: 297.0 };
    let spawn_rot = Vector { x: 0.0, y: 90.0, z: 0.0 };

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

// Pushes char state that the client needs immediately after the scene is ready.
#[instrument(skip(ctx), fields(uid = %ctx.player.uid))]
pub async fn post_load_sync(ctx: &mut NetContext<'_>) -> bool {
    let ok_attrs = super::char_bag::push_char_attrs(ctx).await;
    let ok_status = super::char_bag::push_char_status(ctx).await;
    ok_attrs && ok_status
}
