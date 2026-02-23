use crate::player::LoadingState;
use crate::session::NetContext;
use perlica_proto::{CsLogin, CsPing, ScEnterSceneNotify, ScLogin, ScPing, Vector};
use tracing::debug;

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

        match crate::player::char_bag::prepare_char_bag_sync(ctx.player) {
            Ok(sync_msg) => {
                debug!(
                    "Sending ScSyncCharBagInfo with {} chars, {} teams",
                    sync_msg.char_info.len(),
                    sync_msg.team_info.len()
                );
                let _ = ctx.notify(sync_msg).await;
                let enter_msg = build_enter_scene();
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

fn build_enter_scene() -> ScEnterSceneNotify {
    ScEnterSceneNotify {
        role_id: 1,
        scene_name: "map01_dg001".to_string(),
        scene_id: 37,
        position: Some(Vector {
            x: 756.42,
            y: 95.89,
            z: 137.18,
        }),
    }
}
