use crate::session::NetContext;
use perlica_proto::{
    CsSceneLoadFinish, ScObjectEnterView, ScSelfSceneInfo, SceneCharacter, SceneImplEmpty,
    SceneObjectCommonInfo, SceneObjectDetailContainer, Vector, sc_self_scene_info::SceneImpl,
};
use tracing::debug;

async fn send_sc_object_enter_view(ctx: &mut NetContext<'_>, scene_name: String) {
    let enter_view = ScObjectEnterView {
        scene_name: scene_name.clone(),
        detail: Some(SceneObjectDetailContainer {
            ..Default::default()
        }),
        ..Default::default()
    };
    debug!("Sending ScObjectEnterView for scene {}", scene_name);
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
        x: 756.42,
        y: 95.89,
        z: 137.18,
    };
    let init_rot = Vector {
        x: 0.0,
        y: -20.45,
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
                    position: Some(init_pos),
                    rotation: Some(init_rot),
                    belong_level_script_id: 0,
                    r#type: 0,
                }),
                level: 15,
                name: "Yoshk4e".to_string(),
            }
        })
        .collect();

    debug!("Spawning {} characters", char_list.len());

    send_sc_object_enter_view(ctx, req.scene_name.clone()).await;

    ScSelfSceneInfo {
        scene_name: req.scene_name,
        scene_id: 37,
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
