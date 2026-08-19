use crate::handlers::{bitset, char_bag, factory, mail, mission, scene, unlock, wallet};
use crate::net::NetContext;
use crate::net::crypto::{SessionKeys, rsa_encrypt_key};
use crate::player::LoadingState;
use crate::sconfig;
use common::time::now_ms;
use perlica_logic::character::char_bag::CharBag;
use perlica_proto::{CsLogin, ScLogin, ScReconnectFull, ScSyncBaseData};
use tracing::{debug, info, warn};

pub async fn on_login(ctx: &mut NetContext<'_>, req: CsLogin) -> ScLogin {
    debug!("Login requested: uid={}", req.uid);

    // Reconnection: if a session for this UID is still live — most commonly
    // because the client dropped its TCP connection (backgrounded, switched
    // networks, crashed) and reconnected before the server's read loop
    // noticed the socket was dead — evict it before touching the DB.
    //
    // We hand off rather than just letting both sessions run: two live
    // sessions for one UID would double-register in the `SessionRegistry`
    // and could persist to the DB concurrently, silently clobbering each
    // other's writes. `request_takeover` asks the old session to flush its
    // state, unregister, and close, and only resolves once that has fully
    // happened, so the DB load below is guaranteed to see the old session's
    // final state and there's no window where both sessions are registered.
    let is_reconnect = if let Some(old_session) = ctx.registry.get(&req.uid) {
        info!("Reconnect detected, evicting old session: uid={}", req.uid);
        old_session.request_takeover().await;
        true
    } else {
        false
    };

    ctx.player.on_login(req.uid.clone());
    ctx.player.is_reconnect = is_reconnect;

    let is_new_player = match ctx.db.load(&ctx.player.uid).await {
        Ok(Some(record)) => {
            debug!("Loaded player data from database: uid={}", ctx.player.uid);
            ctx.player.char_bag = record.char_bag;
            ctx.player.world = record.world;
            ctx.player.bitsets = record.bitsets;
            ctx.player.scene.checkpoint = record.checkpoint;
            ctx.player.scene.current_revival_mode = record.revival_mode;
            ctx.player.missions = record.missions;
            ctx.player.guides = record.guides;
            ctx.player.mail = record.mail;
            ctx.player.wallet = record.wallet;
            false
        }
        Ok(None) => {
            let cfg = sconfig::Config::load();
            debug!("Creating new player profile: uid={}", ctx.player.uid);
            ctx.player.char_bag =
                CharBag::new(ctx.assets, cfg.as_ref().unwrap().default_team.members())
                    .unwrap_or_default();
            ctx.player.world = cfg.as_ref().unwrap().world_state.clone();
            true
        }
        Err(error) => {
            let cfg = sconfig::Config::load();
            warn!(
                "Database load failed; using starter data instead: uid={}, error={}",
                ctx.player.uid, error
            );
            ctx.player.char_bag =
                CharBag::new(ctx.assets, &cfg.as_ref().unwrap().default_team.team.clone())
                    .unwrap_or_default();
            true
        }
    };
    ctx.player.is_new_player = is_new_player;
    ctx.player.movement = perlica_logic::movement::MovementManager::from(&ctx.player.world);
    ctx.player
        .scene
        .update_from_world(&ctx.player.world, ctx.assets);

    // Handshake: try to enable ChaCha20 for the rest of the session.
    //
    // We only enable encryption if:
    //   1. The client actually provided a public key in CS_LOGIN (field 8).
    //   2. That key parses as a valid RSA PEM.
    //   3. We can PKCS1v1.5-encrypt our fresh 32-byte ChaCha20 key with it.
    //
    // On any failure we fall back to a plaintext session (is_enc=false) rather
    // than dropping the connection — the server stays usable with legacy
    // clients while modern clients get transport confidentiality.
    let (server_public_key, server_encryp_nonce, is_enc) = if req.client_public_key.is_empty() {
        (Vec::new(), Vec::new(), false)
    } else {
        let keys = SessionKeys::generate();
        match rsa_encrypt_key(&req.client_public_key, &keys.key) {
            Ok(encrypted_key) => {
                // Arm ciphers on the context — the actual activation happens
                // AFTER the SC_LOGIN frame is written (outbound) and AFTER the
                // CS_LOGIN frame's handler returns (inbound), so SC_LOGIN
                // itself is transmitted in the clear as the spec requires.
                //
                // Both directions are seeded from the same key/nonce but keep
                // independent cipher instances so their block counters never
                // collide.
                ctx.arm_session_ciphers(keys.cipher(), keys.cipher());
                debug!("ChaCha20 handshake armed: uid={}", ctx.player.uid);
                (encrypted_key, keys.nonce.to_vec(), true)
            }
            Err(e) => {
                warn!(
                    "Handshake failed, falling back to plaintext: uid={}, error={}",
                    ctx.player.uid, e
                );
                (Vec::new(), Vec::new(), false)
            }
        }
    };

    ScLogin {
        uid: req.uid,
        is_first_login: false,
        server_public_key,
        server_encryp_nonce,
        last_recv_up_seqid: ctx.client_seq_id,
        is_reconnect,
        is_enc,
        // The client sets `last_recv_down_seqid` on CS_LOGIN when it believes
        // it's resuming an existing session rather than starting fresh.
        is_client_reconnect: req.last_recv_down_seqid > 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginPhase {
    BaseData,
    Wallet,
    ItemBag,
    CharBag,
    Unlocks,
    Guides,
    Missions,
    CharAttrs,
    CharStatus,
    Factory,
    Bitsets,
    Mail,
    RoleSceneInfo,
    EnterScene,
    Done,
}

impl LoginPhase {
    fn next(self) -> Self {
        match self {
            Self::BaseData => Self::Wallet,
            Self::Wallet => Self::ItemBag,
            Self::ItemBag => Self::CharBag,
            Self::CharBag => Self::Unlocks,
            Self::Unlocks => Self::Guides,
            Self::Guides => Self::Missions,
            Self::Missions => Self::CharAttrs,
            Self::CharAttrs => Self::CharStatus,
            Self::CharStatus => Self::Factory,
            Self::Factory => Self::Bitsets,
            Self::Bitsets => Self::Mail,
            Self::Mail => Self::RoleSceneInfo,
            Self::RoleSceneInfo => Self::EnterScene,
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
            debug!("Login sequence complete: uid={}", ctx.player.uid);

            // The phases above already pushed a full, fresh snapshot of every
            // subsystem (wallet, bags, char bag, missions, factory, ...), so
            // a reconnect is naturally served as a full resync rather than an
            // incremental replay — there's no seq-tracked delta log to draw
            // an SC_RECONNECT_INCR from. Tell the client explicitly so it
            // knows to treat what it just received as authoritative state
            // rather than diffing against what it had before the drop.
            if ctx.player.is_reconnect
                && let Err(e) = ctx.notify(ScReconnectFull {}).await
            {
                warn!(
                    "Failed to send SC_RECONNECT_FULL: uid={}, error={}",
                    ctx.player.uid, e
                );
            }
            break;
        }
        debug!(
            "Login sequence phase: uid={}, phase={:?}",
            ctx.player.uid, phase
        );
        let ok = match phase {
            LoginPhase::BaseData => push_base_data(ctx).await,
            LoginPhase::Wallet => wallet::push_wallet(ctx).await,
            LoginPhase::ItemBag => char_bag::push_item_bag_sync(ctx).await,
            LoginPhase::CharBag => char_bag::push_char_bag(ctx).await,
            LoginPhase::Unlocks => unlock::push_unlocks(ctx).await,
            LoginPhase::Guides => mission::push_guides(ctx).await,
            LoginPhase::Missions => mission::push_missions(ctx).await,
            LoginPhase::CharAttrs => char_bag::push_char_attrs(ctx).await,
            LoginPhase::CharStatus => char_bag::push_char_status(ctx).await,
            LoginPhase::Factory => factory::push_factory(ctx).await,
            LoginPhase::Bitsets => bitset::push_bitsets(ctx).await,
            LoginPhase::Mail => {
                let sync_ok = mail::push_mail_sync(ctx).await;
                if sync_ok {
                    mail::deliver_login_mails(ctx, ctx.player.is_new_player).await;
                }
                sync_ok
            }
            LoginPhase::RoleSceneInfo => unlock::all_role_sync(ctx).await,
            LoginPhase::EnterScene => scene::notify_enter_scene(ctx).await,
            LoginPhase::Done => unreachable!(),
        };
        if ok {
            phase = phase.next();
        } else {
            warn!(
                "Login sequence failed: uid={}, phase={:?}",
                ctx.player.uid, phase
            );
        }
    }
}

async fn push_base_data(ctx: &mut NetContext<'_>) -> bool {
    ctx.notify(ScSyncBaseData {
        roleid: 1,
        role_name: "BeyondDefault".to_string(),
        level: ctx.player.world.role_level as u32,
        exp: ctx.player.world.role_exp as u32,
        server_time: now_ms() as i64,
        server_time_zone: 0,
    })
    .await
    .is_ok()
}
