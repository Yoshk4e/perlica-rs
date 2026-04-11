use crate::net::NetContext;
use perlica_logic::character::char_bag::{
    handle_weapon_add_exp, handle_weapon_attach_gem, handle_weapon_breakthrough,
    handle_weapon_detach_gem, handle_weapon_puton,
};
use perlica_proto::{
    CsWeaponAddExp, CsWeaponAttachGem, CsWeaponBreakthrough, CsWeaponDetachGem, CsWeaponPuton,
    ScWeaponAddExp, ScWeaponAttachGem, ScWeaponBreakthrough, ScWeaponDetachGem, ScWeaponPuton,
};
use tracing::{debug, error};

/// Equips a weapon, unequipping it from its previous owner first. Returns zero `weaponid` on failure.
pub async fn on_cs_weapon_puton(ctx: &mut NetContext<'_>, req: CsWeaponPuton) -> ScWeaponPuton {
    debug!(
        "Weapon put-on request: uid={}, char_id={}, weapon_id={}",
        ctx.player.uid, req.charid, req.weaponid
    );

    let response = handle_weapon_puton(&mut ctx.player.char_bag, req.charid, req.weaponid);

    if let Err(error) = &response {
        error!(
            "Weapon put-on failed: uid={}, char_id={}, weapon_id={}, error={:?}",
            ctx.player.uid, req.charid, req.weaponid, error
        );
    }

    response.unwrap_or_else(|_| ScWeaponPuton {
        charid: req.charid,
        weaponid: 0,
        offweaponid: 0,
        put_off_charid: 0,
    })
}

/// Feeds fodder weapons into a target weapon. Consumed weapons are removed from the depot.
pub async fn on_cs_weapon_add_exp(ctx: &mut NetContext<'_>, req: CsWeaponAddExp) -> ScWeaponAddExp {
    debug!(
        "Weapon add-exp request: uid={}, weapon_id={}, fodder_count={}",
        ctx.player.uid,
        req.weaponid,
        req.cost_weapon_ids.len()
    );

    let response = handle_weapon_add_exp(
        &mut ctx.player.char_bag,
        req.weaponid,
        &req.cost_weapon_ids,
        ctx.assets,
    );

    if let Err(error) = &response {
        error!(
            "Weapon add-exp failed: uid={}, weapon_id={}, error={:?}",
            ctx.player.uid, req.weaponid, error
        );
    }

    response.unwrap_or_else(|_| ScWeaponAddExp {
        weaponid: req.weaponid,
        new_exp: 0,
        weapon_lv: 1,
    })
}

/// Advances breakthrough level by one. Weapon must be at its current level cap.
pub async fn on_cs_weapon_breakthrough(
    ctx: &mut NetContext<'_>,
    req: CsWeaponBreakthrough,
) -> ScWeaponBreakthrough {
    debug!(
        "Weapon breakthrough request: uid={}, weapon_id={}",
        ctx.player.uid, req.weaponid
    );

    let response = handle_weapon_breakthrough(&mut ctx.player.char_bag, req.weaponid, ctx.assets);

    if let Err(error) = &response {
        error!(
            "Weapon breakthrough failed: uid={}, weapon_id={}, error={:?}",
            ctx.player.uid, req.weaponid, error
        );
    }

    response.unwrap_or_else(|_| ScWeaponBreakthrough {
        weaponid: req.weaponid,
        breakthrough_lv: 1,
    })
}

/// Attaches a gem, detaching it from any previous weapon first. Previous gem on target is echoed in `detach_gemid`.
pub async fn on_cs_weapon_attach_gem(
    ctx: &mut NetContext<'_>,
    req: CsWeaponAttachGem,
) -> ScWeaponAttachGem {
    debug!(
        "Weapon attach-gem request: uid={}, weapon_id={}, gem_id={}",
        ctx.player.uid, req.weaponid, req.gemid
    );

    let response = handle_weapon_attach_gem(&mut ctx.player.char_bag, req.weaponid, req.gemid);

    if let Err(error) = &response {
        error!(
            "Weapon attach-gem failed: uid={}, weapon_id={}, gem_id={}, error={:?}",
            ctx.player.uid, req.weaponid, req.gemid, error
        );
    }

    response.unwrap_or_else(|_| ScWeaponAttachGem {
        weaponid: req.weaponid,
        gemid: 0,
        detach_gemid: 0,
        detach_gem_weaponid: 0,
    })
}

/// Removes the socketed gem; detached ID is echoed in `detach_gemid`.
pub async fn on_cs_weapon_detach_gem(
    ctx: &mut NetContext<'_>,
    req: CsWeaponDetachGem,
) -> ScWeaponDetachGem {
    debug!(
        "Weapon detach-gem request: uid={}, weapon_id={}",
        ctx.player.uid, req.weaponid
    );

    let response = handle_weapon_detach_gem(&mut ctx.player.char_bag, req.weaponid);

    if let Err(error) = &response {
        error!(
            "Weapon detach-gem failed: uid={}, weapon_id={}, error={:?}",
            ctx.player.uid, req.weaponid, error
        );
    }

    response.unwrap_or_else(|_| ScWeaponDetachGem {
        weaponid: req.weaponid,
        detach_gemid: 0,
    })
}
