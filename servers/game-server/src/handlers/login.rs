use crate::handlers::{char_bag, factory, scene, unlock};
use crate::net::NetContext;
use crate::player::LoadingState;
use common::time::now_ms;
use perlica_proto::{CsLogin, ScLogin, ScSyncBaseData};
use tracing::{debug, instrument, warn};

#[instrument(skip(ctx), fields(uid = %req.uid))]
pub async fn on_login(ctx: &mut NetContext<'_>, req: CsLogin) -> ScLogin {
    ctx.player.on_login(req.uid.clone());
    debug!("login");

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

// Drives the post-login state machine, advancing one step per iteration until
// the player reaches EnterScene or a step fails.
#[instrument(skip(ctx), fields(uid = %ctx.player.uid, state = ?ctx.player.loading_state))]
pub(crate) async fn run_login_sequence(ctx: &mut NetContext<'_>) {
    loop {
        let ok = match ctx.player.loading_state {
            LoadingState::ScLogin => {
                push_base_data(ctx).await && char_bag::push_char_bag(ctx).await
            }
            LoadingState::CharBagSync => {
                unlock::push_unlocks(ctx).await
                    && char_bag::push_char_attrs(ctx).await
                    && char_bag::push_char_status(ctx).await
            }
            LoadingState::UnlockSync => factory::push_factory(ctx).await,
            LoadingState::FactorySync => scene::notify_enter_scene(ctx).await,
            LoadingState::EnterScene | LoadingState::Complete | LoadingState::Login => break,
        };

        if ok {
            ctx.player.advance_state();
        } else {
            warn!(state = ?ctx.player.loading_state, "login sequence step failed");
            break;
        }
    }
}

#[instrument(skip(ctx), fields(uid = %ctx.player.uid))]
async fn push_base_data(ctx: &mut NetContext<'_>) -> bool {
    let uid: u64 = ctx.player.uid.parse().unwrap_or(1);
    let msg = ScSyncBaseData {
        roleid: uid,
        role_name: ctx.player.uid.clone(),
        level: 1,
        exp: 0,
        server_time: now_ms() as i64,
        server_time_zone: 0,
    };
    debug!(roleid = uid, "base data");
    ctx.notify(msg).await.is_ok()
}
