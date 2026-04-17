//! Scene enter / load-finish handshake.

use crate::net::NetContext;
use perlica_proto::{CsSceneLoadFinish, ScEnterSceneNotify, ScSelfSceneInfo, Vector};
use tracing::{debug, error, info};

/// Pushes `ScEnterSceneNotify` during login before the client has finished loading.
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
            x: ctx.player.movement.pos_x,
            y: ctx.player.movement.pos_y,
            z: ctx.player.movement.pos_z,
        }),
    };

    debug!("Entering scene: {}", msg.scene_name);

    if let Err(error) = ctx.notify(msg).await {
        error!("Failed to send enter scene notification: {:?}", error);
        return false;
    }

    true
}

/// Handles `CsSceneLoadFinish`. Finalises scene state and syncs all entities and character state.
pub async fn on_scene_load_finish(
    ctx: &mut NetContext<'_>,
    req: CsSceneLoadFinish,
) -> ScSelfSceneInfo {
    info!("Scene load finished: {}", req.scene_name);

    ctx.player.world.last_scene = req.scene_name.clone();

    let (enter_view, self_info) = ctx.player.scene.finish_scene_load(
        &ctx.player.char_bag,
        &ctx.player.movement,
        ctx.assets,
        &mut ctx.player.entities,
    );

    if let Err(error) = ctx.notify(enter_view).await {
        error!("Failed to send object enter view: {:?}", error);
    }

    let pos = ctx.player.movement.position_tuple();
    let (initial_enter, _) =
        ctx.player
            .scene
            .update_visible_entities(pos, ctx.assets, &mut ctx.player.entities);

    if let Some(msg) = initial_enter {
        let _ = ctx.notify(msg).await.inspect_err(|error| {
            error!("Failed to send initial dynamic enter view: {:?}", error);
        });
    }

    if !crate::handlers::factory::push_factory(ctx).await {
        error!("Failed to sync factory context");
    }

    if !post_load_sync(ctx).await {
        error!("Failed to complete post-load sync");
    }

    self_info
}

async fn post_load_sync(ctx: &mut NetContext<'_>) -> bool {
    let ok_attrs = crate::handlers::char_bag::push_char_attrs(ctx).await;
    let ok_status = crate::handlers::char_bag::push_char_status(ctx).await;
    ok_attrs && ok_status
}
