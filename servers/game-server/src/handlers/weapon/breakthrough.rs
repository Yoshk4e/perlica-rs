use crate::net::NetContext;
use perlica_logic::character::char_bag::handle_weapon_breakthrough;
use perlica_proto::{CsWeaponBreakthrough, ScWeaponBreakthrough};
use tracing::{debug, error};

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

    if let Err(ref e) = response {
        error!(
            "Weapon breakthrough failed: uid={}, weapon_id={}, error={:?}",
            ctx.player.uid, req.weaponid, e
        );
    }

    response.unwrap_or(ScWeaponBreakthrough {
        weaponid: req.weaponid,
        breakthrough_lv: 1,
    })
}
