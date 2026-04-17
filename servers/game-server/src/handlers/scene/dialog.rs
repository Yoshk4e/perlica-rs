use crate::net::NetContext;
use perlica_proto::{CsFinishDialog, ScFinishDialog};
use tracing::info;

pub async fn on_cs_finish_dialog(ctx: &mut NetContext<'_>, req: CsFinishDialog) -> ScFinishDialog {
    info!(
        "Dialog finished: dialog_id={}, trunks={:?}",
        req.dialog_id, req.trunk_id_list
    );

    let scene_name = ctx.player.scene.scene_name().to_string();
    let activated = ctx
        .player
        .scene
        .level_scripts
        .on_dialog_finished(&scene_name, ctx.assets);
    for script_id in activated {
        if let Some(notify) = ctx
            .player
            .scene
            .level_scripts
            .state_notify(&scene_name, script_id)
        {
            let _ = ctx.notify(notify).await;
        }
    }

    ScFinishDialog {
        dialog_id: req.dialog_id,
        trunk_id_list: req.trunk_id_list,
    }
}
