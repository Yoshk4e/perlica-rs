use crate::net::NetContext;
use perlica_logic::character::char_bag::handle_weapon_puton;
use perlica_proto::{CsWeaponPuton, ScWeaponPuton};
use tracing::{debug, error};

/// Equips a weapon, unequipping it from its previous owner first.
/// Returns zero `weaponid` on failure.
pub async fn on_cs_weapon_puton(ctx: &mut NetContext<'_>, req: CsWeaponPuton) -> ScWeaponPuton {
    debug!(
        "Weapon put-on request: uid={}, char_id={}, weapon_id={}",
        ctx.player.uid, req.charid, req.weaponid
    );

    let response = handle_weapon_puton(&mut ctx.player.char_bag, req.charid, req.weaponid);

    if let Err(ref e) = response {
        error!(
            "Weapon put-on failed: uid={}, char_id={}, weapon_id={}, error={:?}",
            ctx.player.uid, req.charid, req.weaponid, e
        );
    }

    response.unwrap_or(ScWeaponPuton {
        charid: req.charid,
        weaponid: 0,
        offweaponid: 0,
        put_off_charid: 0,
    })
}
