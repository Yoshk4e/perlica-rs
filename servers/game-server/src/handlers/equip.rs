use crate::net::NetContext;
use config::item::{CraftShowingType, ItemDepotType};
use perlica_logic::item::{EquipInstId, GemInstId, WeaponInstId};
use perlica_proto::{
    CsEquipPutoff, CsEquipPuton, CsRemoveItemNewTags, ScEquipPutoff, ScEquipPuton,
    ScRemoveItemNewTags,
};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Equips a piece to a character's slot.
///
/// On failure echoes back `equipid = 0` so the client can roll back its optimistic update.
pub async fn on_cs_equip_puton(ctx: &mut NetContext<'_>, req: CsEquipPuton) -> ScEquipPuton {
    debug!(
        "EquipPuton: uid={}, char_id={}, slot={}, equip_id={}",
        ctx.player.uid, req.charid, req.slotid, req.equipid
    );

    let inst_id = EquipInstId::new(req.equipid);

    if ctx
        .player
        .char_bag
        .item_manager
        .equips
        .get(inst_id)
        .is_none()
    {
        warn!(
            "EquipPuton rejected: equip inst {} not found, uid={}",
            req.equipid, ctx.player.uid
        );
        return ScEquipPuton {
            charid: req.charid,
            slotid: req.slotid,
            equipid: 0,
            suitinfo: HashMap::new(),
            put_off_charid: 0,
            old_owner_suitinfo: HashMap::new(),
        };
    }

    match ctx
        .player
        .char_bag
        .item_manager
        .equips
        .equip(inst_id, req.charid)
    {
        Ok(displaced) => {
            info!(
                "EquipPuton: uid={}, char={}, equip={}, displaced={:?}",
                ctx.player.uid, req.charid, req.equipid, displaced
            );
            ScEquipPuton {
                charid: req.charid,
                slotid: req.slotid,
                equipid: req.equipid,
                suitinfo: HashMap::new(),
                put_off_charid: 0,
                old_owner_suitinfo: HashMap::new(),
            }
        }
        Err(e) => {
            error!(
                "EquipPuton failed: uid={}, char={}, equip={}, err={:?}",
                ctx.player.uid, req.charid, req.equipid, e
            );
            ScEquipPuton {
                charid: req.charid,
                slotid: req.slotid,
                equipid: 0,
                suitinfo: HashMap::new(),
                put_off_charid: 0,
                old_owner_suitinfo: HashMap::new(),
            }
        }
    }
}

/// Unequips the piece in `slotid` from a character.
pub async fn on_cs_equip_putoff(ctx: &mut NetContext<'_>, req: CsEquipPutoff) -> ScEquipPutoff {
    debug!(
        "EquipPutoff: uid={}, char_id={}, slot={}",
        ctx.player.uid, req.charid, req.slotid
    );

    let slot = CraftShowingType::try_from(req.slotid as u32).unwrap_or(CraftShowingType::None);

    let inst_id = ctx
        .player
        .char_bag
        .item_manager
        .equips
        .get_in_slot(req.charid, slot)
        .map(|p| p.inst_id);

    match inst_id {
        None => {
            warn!(
                "EquipPutoff: no equip in slot {:?} for char {}, uid={}",
                slot, req.charid, ctx.player.uid
            );
        }
        Some(id) => {
            if let Err(e) = ctx.player.char_bag.item_manager.equips.unequip(id) {
                error!(
                    "EquipPutoff failed: uid={}, char={}, slot={:?}, err={:?}",
                    ctx.player.uid, req.charid, slot, e
                );
            } else {
                info!(
                    "EquipPutoff: uid={}, char={}, slot={:?}, inst={}",
                    ctx.player.uid,
                    req.charid,
                    slot,
                    id.as_u64()
                );
            }
        }
    }

    ScEquipPutoff {
        charid: req.charid,
        slotid: req.slotid,
        suitinfo: HashMap::new(),
    }
}

/// Clears the `is_new` flag on instanced items so the client stops showing the new-item badge.
pub async fn on_cs_remove_item_new_tags(
    ctx: &mut NetContext<'_>,
    req: CsRemoveItemNewTags,
) -> ScRemoveItemNewTags {
    debug!(
        "RemoveItemNewTags: uid={}, entries={}",
        ctx.player.uid,
        req.inst_data.len()
    );

    for entry in &req.inst_data {
        let depot =
            ItemDepotType::try_from(entry.depot_type as u32).unwrap_or(ItemDepotType::Invalid);

        match depot {
            ItemDepotType::Weapon => {
                for &raw_id in &entry.inst_ids {
                    let id = WeaponInstId::new(raw_id);
                    if let Err(e) = ctx.player.char_bag.item_manager.weapons.clear_new_flag(id) {
                        warn!(
                            "RemoveItemNewTags: weapon {} not found (uid={}): {:?}",
                            raw_id, ctx.player.uid, e
                        );
                    }
                }
            }
            ItemDepotType::WeaponGem => {
                for &raw_id in &entry.inst_ids {
                    let id = GemInstId::new(raw_id);
                    if let Err(e) = ctx.player.char_bag.item_manager.gems.clear_new_flag(id) {
                        warn!(
                            "RemoveItemNewTags: gem {} not found (uid={}): {:?}",
                            raw_id, ctx.player.uid, e
                        );
                    }
                }
            }
            ItemDepotType::Equip => {
                for &raw_id in &entry.inst_ids {
                    let id = EquipInstId::new(raw_id);
                    if let Err(e) = ctx.player.char_bag.item_manager.equips.clear_new_flag(id) {
                        warn!(
                            "RemoveItemNewTags: equip {} not found (uid={}): {:?}",
                            raw_id, ctx.player.uid, e
                        );
                    }
                }
            }
            other => {
                debug!(
                    "RemoveItemNewTags: depot {:?} is stackable or unknown, skipping",
                    other
                );
            }
        }
    }

    ScRemoveItemNewTags {}
}
