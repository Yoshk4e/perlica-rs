use crate::handlers::{bitset, char_bag, factory, scene, unlock};
use crate::net::NetContext;
use crate::player::LoadingState;
use common::time::now_ms;
use perlica_logic::character::char_bag::CharBag;
use perlica_proto::{CsLogin, ScLogin, ScSyncBaseData};
use tracing::{debug, warn};

pub async fn on_login(ctx: &mut NetContext<'_>, req: CsLogin) -> ScLogin {
    ctx.player.on_login(req.uid.clone());
    debug!(uid = %req.uid, "login");

    match ctx.db.load(&ctx.player.uid).await {
        Ok(Some(record)) => {
            debug!(uid = %ctx.player.uid, "loaded from db");
            ctx.player.char_bag = record.char_bag;
            ctx.player.world = record.world;
        }
        Ok(None) => {
            debug!(uid = %ctx.player.uid, "new player");
            ctx.player.char_bag = CharBag::new(ctx.assets).unwrap_or_default();
        }
        Err(e) => {
            warn!(error = %e, "db load failed, using starter");
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

// ---------------------------------------------------------------------------
// Login sequence state machine
//
// Inspired by the Zig server's event queue: each phase is an explicit named
// state that knows its successor. The machine drives itself forward, logging
// which step it's on, and stops on the first failure.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginPhase {
    BaseData,
    ItemBag,
    CharBag,
    Unlocks,
    CharAttrs,
    CharStatus,
    Factory,
    Bitsets,
    EnterScene,
    Done,
}

impl LoginPhase {
    fn next(self) -> Self {
        match self {
            Self::BaseData => Self::ItemBag,
            Self::ItemBag => Self::CharBag,
            Self::CharBag => Self::Unlocks,
            Self::Unlocks => Self::CharAttrs,
            Self::CharAttrs => Self::CharStatus,
            Self::CharStatus => Self::Factory,
            Self::Factory => Self::Bitsets,
            Self::Bitsets => Self::EnterScene,
            Self::EnterScene => Self::Done,
            Self::Done => Self::Done,
        }
    }
}

pub(crate) async fn run_login_sequence(ctx: &mut NetContext<'_>) {
    let mut phase = LoginPhase::BaseData;

    loop {
        if phase == LoginPhase::Done {
            ctx.player.loading_state = LoadingState::Complete;
            debug!(uid = %ctx.player.uid, "login sequence complete");
            break;
        }

        debug!(uid = %ctx.player.uid, phase = ?phase, "login phase");

        let ok = match phase {
            LoginPhase::BaseData => push_base_data(ctx).await,
            LoginPhase::ItemBag => char_bag::push_item_bag_sync(ctx).await,
            LoginPhase::CharBag => char_bag::push_char_bag(ctx).await,
            LoginPhase::Unlocks => unlock::push_unlocks(ctx).await,
            LoginPhase::CharAttrs => char_bag::push_char_attrs(ctx).await,
            LoginPhase::CharStatus => char_bag::push_char_status(ctx).await,
            LoginPhase::Factory => factory::push_factory(ctx).await,
            LoginPhase::Bitsets => bitset::push_bitsets(ctx).await,
            LoginPhase::EnterScene => scene::notify_enter_scene(ctx).await,
            LoginPhase::Done => unreachable!(),
        };

        if ok {
            phase = phase.next();
        } else {
            warn!(uid = %ctx.player.uid, phase = ?phase, "login sequence failed");
            break;
        }
    }
}

async fn push_base_data(ctx: &mut NetContext<'_>) -> bool {
    let uid: u64 = ctx.player.uid.parse().unwrap_or(1);
    ctx.notify(ScSyncBaseData {
        roleid: uid,
        role_name: ctx.player.uid.clone(),
        level: ctx.player.world.role_level as u32,
        exp: ctx.player.world.role_exp as u32,
        server_time: now_ms() as i64,
        server_time_zone: 0,
    })
    .await
    .is_ok()
}
