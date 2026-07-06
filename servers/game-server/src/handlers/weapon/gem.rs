use crate::net::NetContext;
use perlica_logic::character::char_bag::{handle_weapon_attach_gem, handle_weapon_detach_gem};
use perlica_proto::{CsWeaponAttachGem, CsWeaponDetachGem, ScWeaponAttachGem, ScWeaponDetachGem};
use tracing::{debug, error, warn};

/// Attaches a gem. Previous gem on target is echoed in `detach_gemid`.
pub async fn on_cs_weapon_attach_gem(
    ctx: &mut NetContext<'_>,
    req: CsWeaponAttachGem,
) -> ScWeaponAttachGem {
    debug!(
        "Weapon attach-gem request: uid={}, weapon_id={}, gem_id={}",
        ctx.player.uid, req.weaponid, req.gemid
    );

    let response = handle_weapon_attach_gem(&mut ctx.player.char_bag, req.weaponid, req.gemid);

    if let Err(ref e) = response {
        error!(
            "Weapon attach-gem failed: uid={}, weapon_id={}, gem_id={}, error={:?}",
            ctx.player.uid, req.weaponid, req.gemid, e
        );
    }

    if response.is_ok()
        && let Err(e) = ctx
            .db
            .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
            .await
    {
        warn!(
            "Failed to persist char_bag after weapon attach gem: uid={}, error={}",
            ctx.player.uid, e
        );
    }

    response.unwrap_or(ScWeaponAttachGem {
        weaponid: req.weaponid,
        gemid: 0,
        detach_gemid: 0,
        detach_gem_weaponid: 0,
    })
}

/// Removes the socketed gem.
pub async fn on_cs_weapon_detach_gem(
    ctx: &mut NetContext<'_>,
    req: CsWeaponDetachGem,
) -> ScWeaponDetachGem {
    debug!(
        "Weapon detach-gem request: uid={}, weapon_id={}",
        ctx.player.uid, req.weaponid
    );

    let response = handle_weapon_detach_gem(&mut ctx.player.char_bag, req.weaponid);

    if let Err(ref e) = response {
        error!(
            "Weapon detach-gem failed: uid={}, weapon_id={}, error={:?}",
            ctx.player.uid, req.weaponid, e
        );
    }

    if response.is_ok()
        && let Err(e) = ctx
            .db
            .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
            .await
    {
        warn!(
            "Failed to persist char_bag after weapon detach gem: uid={}, error={}",
            ctx.player.uid, e
        );
    }

    response.unwrap_or(ScWeaponDetachGem {
        weaponid: req.weaponid,
        detach_gemid: 0,
    })
}
