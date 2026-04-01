use crate::handlers::{bitset, char_bag, factory, mission, scene, unlock};
use crate::net::NetContext;
use crate::player::LoadingState;
use crate::sconfig;
use common::time::now_ms;
use perlica_logic::character::char_bag::CharBag;
use perlica_proto::{CsLogin, ScLogin, ScSyncBaseData};
use tracing::{debug, warn};

pub async fn on_login(ctx: &mut NetContext<'_>, req: CsLogin) -> ScLogin {
    ctx.player.on_login(req.uid.clone());
    debug!("Login requested: uid={}", req.uid);

    match ctx.db.load(&ctx.player.uid).await {
        Ok(Some(record)) => {
            debug!("Loaded player data from database: uid={}", ctx.player.uid);
            ctx.player.char_bag = record.char_bag;
            ctx.player.world = record.world;
            ctx.player.bitsets = record.bitsets;
            ctx.player.scene.checkpoint = record.checkpoint;
            ctx.player.scene.current_revival_mode = record.revival_mode;
            ctx.player.missions = record.missions;
            ctx.player.guides = record.guides;
        }
        Ok(None) => {
            let cfg = sconfig::Config::load();
            debug!("Creating new player profile: uid={}", ctx.player.uid);
            ctx.player.char_bag =
                CharBag::new(ctx.assets, &cfg.as_ref().unwrap().default_team.team.clone())
                    .unwrap_or_default();
            ctx.player.world = cfg.as_ref().unwrap().world_state.clone();
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
        }
    }

    ctx.player.movement = perlica_logic::movement::MovementManager::from_world(&ctx.player.world);
    ctx.player
        .scene
        .update_from_world(&ctx.player.world, ctx.assets);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginPhase {
    BaseData,
    ItemBag,
    CharBag,
    Unlocks,
    Guides,
    Missions,
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
            Self::Unlocks => Self::Guides,
            Self::Guides => Self::Missions,
            Self::Missions => Self::CharAttrs,
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
            debug!("Login sequence complete: uid={}", ctx.player.uid);
            break;
        }

        debug!(
            "Login sequence phase: uid={}, phase={:?}",
            ctx.player.uid, phase
        );

        let ok = match phase {
            LoginPhase::BaseData => push_base_data(ctx).await,
            LoginPhase::ItemBag => char_bag::push_item_bag_sync(ctx).await,
            LoginPhase::CharBag => char_bag::push_char_bag(ctx).await,
            LoginPhase::Unlocks => unlock::push_unlocks(ctx).await,
            LoginPhase::Guides => mission::push_guides(ctx).await,
            LoginPhase::Missions => mission::push_missions(ctx).await,
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
            warn!(
                "Login sequence failed: uid={}, phase={:?}",
                ctx.player.uid, phase
            );
            break;
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
