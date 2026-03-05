use crate::handlers::{bitset, char_bag, factory, scene, unlock};
use crate::net::NetContext;
use crate::player::LoadingState;
use common::time::now_ms;
use perlica_logic::character::char_bag::CharBag;
use perlica_proto::{CsLogin, ScLogin, ScSyncBaseData};
use tracing::{debug, error, instrument, warn};

#[instrument(skip(ctx), fields(uid = %req.uid))]
pub async fn on_login(ctx: &mut NetContext<'_>, req: CsLogin) -> ScLogin {
    ctx.player.on_login(req.uid.clone());

    match ctx.db.load(&ctx.player.uid).await {
        Ok(Some(record)) => {
            debug!("Loaded Player with UID {} from db", ctx.player.uid);
            ctx.player.char_bag = record.char_bag;
            ctx.player.world = record.world;
        }
        Ok(None) => {
            debug!("new player, initializing");
            ctx.player.char_bag = CharBag::new(ctx.assets).unwrap_or_default();
        }
        Err(e) => {
            error!(error = %e, "db load failed, falling back to starter");
            ctx.player.char_bag = CharBag::new(ctx.assets).unwrap_or_default();
        }
    }

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
            LoadingState::UnlockSync => {
                factory::push_factory(ctx).await && bitset::push_bitsets(ctx).await
            }
            LoadingState::FactorySync => scene::notify_enter_scene(ctx).await,
            LoadingState::EnterScene | LoadingState::Complete | LoadingState::Login => break,
        };

        if ok {
            ctx.player.advance_state();
        } else {
            warn!("login sequence step failed");
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
        level: ctx.player.world.role_level as u32,
        exp: ctx.player.world.role_exp as u32,
        server_time: now_ms() as i64,
        server_time_zone: 0,
    };
    debug!(roleid = uid, level = msg.level, "base data");
    ctx.notify(msg).await.is_ok()
}
