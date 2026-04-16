use crate::net::NetContext;
use perlica_logic::{
    character::char_bag::{
        handle_weapon_attach_gem, handle_weapon_breakthrough, handle_weapon_detach_gem,
        handle_weapon_puton,
    },
    item::WeaponInstId,
};
use perlica_proto::{
    CsWeaponAddExp, CsWeaponAttachGem, CsWeaponBreakthrough, CsWeaponDetachGem, CsWeaponPuton,
    ScItemBagSyncModify, ScWeaponAddExp, ScWeaponAttachGem, ScWeaponBreakthrough,
    ScWeaponDetachGem, ScWeaponPuton, ScdItemDepotModify,
};
use std::collections::HashMap;
use tracing::{debug, error, warn};

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

pub async fn on_cs_weapon_add_exp(ctx: &mut NetContext<'_>, req: CsWeaponAddExp) -> ScWeaponAddExp {
    let target_id = WeaponInstId::new(req.weaponid);

    let (template_id, current_level, current_exp) = {
        let Some(weapon_data) = ctx.player.char_bag.item_manager.weapons.get(target_id) else {
            warn!("WeaponAddExp failed: unknown weapon_id={}", req.weaponid);
            return ScWeaponAddExp {
                weaponid: req.weaponid,
                new_exp: 0,
                weapon_lv: 1,
            };
        };
        (
            weapon_data.template_id.clone(),
            weapon_data.weapon_lv,
            weapon_data.exp,
        )
    };

    let Some(weapon_template) = ctx.assets.weapons.get(&template_id) else {
        return ScWeaponAddExp {
            weaponid: req.weaponid,
            new_exp: current_exp,
            weapon_lv: current_level,
        };
    };

    let max_level = weapon_template.max_lv as u64;
    if current_level >= max_level {
        return ScWeaponAddExp {
            weaponid: req.weaponid,
            new_exp: current_exp,
            weapon_lv: current_level,
        };
    }

    let upgrade_sum_template = ctx
        .assets
        .weapons
        .get_upgrade_sum(&weapon_template.level_template_id);
    if upgrade_sum_template.is_none() {
        return ScWeaponAddExp {
            weaponid: req.weaponid,
            new_exp: current_exp,
            weapon_lv: current_level,
        };
    }
    let upgrade_sum_list = &upgrade_sum_template.unwrap().list;

    let mut total_exp_gained: i64 = 0;
    let mut consumed_stackable: HashMap<String, u32> = HashMap::new();

    use config::item::ItemDepotType;

    for (item_id, &count) in &req.cost_item_id2_count {
        if count == 0 {
            continue;
        }
        let count = count as u32;
        let exp_per_unit = ctx.assets.weapons.weapon_exp_for_item(item_id);
        if exp_per_unit == 0 {
            continue;
        }

        let consumed_ok = ctx
            .player
            .char_bag
            .item_manager
            .consume_stackable(ItemDepotType::SpecialItem, item_id, count)
            .is_ok()
            || ctx
                .player
                .char_bag
                .item_manager
                .consume_stackable(ItemDepotType::Factory, item_id, count)
                .is_ok();

        if consumed_ok {
            total_exp_gained += exp_per_unit as i64 * count as i64;
            *consumed_stackable.entry(item_id.clone()).or_insert(0) += count;
        }
    }

    let mut valid_fodders: Vec<WeaponInstId> = Vec::new();
    let mut fodder_exp: i64 = 0;

    for &fid in &req.cost_weapon_ids {
        let fid = WeaponInstId::new(fid);
        if fid == target_id {
            continue;
        }

        if let Some(f) = ctx.player.char_bag.item_manager.weapons.get(fid) {
            if f.is_lock || f.is_equipped() {
                continue;
            }

            let base: i64 = ctx
                .assets
                .weapons
                .get(&f.template_id)
                .map(|w| match w.rarity {
                    6 => 5000,
                    5 => 3000,
                    4 => 1500,
                    3 => 800,
                    _ => 400,
                })
                .unwrap_or(400);

            fodder_exp += base + (f.weapon_lv as f64 * 0.1 * base as f64) as i64;
            valid_fodders.push(fid);
        }
    }

    for &fid in &valid_fodders {
        let _ = ctx.player.char_bag.item_manager.weapons.remove_weapon(fid);
    }

    total_exp_gained += fodder_exp;

    let (final_lv, final_exp) = if total_exp_gained > 0 {
        let cum_at_current = upgrade_sum_list
            .iter()
            .find(|item| item.weapon_lv as u64 == current_level)
            .map(|item| item.lv_up_exp_sum as i64)
            .unwrap_or(0);

        let new_total_exp = cum_at_current + (current_exp as i64) + total_exp_gained;
        let mut new_level = current_level;
        let mut final_cum_sum = cum_at_current;

        for item in upgrade_sum_list {
            if item.weapon_lv as u64 > max_level {
                break;
            }
            if new_total_exp >= item.lv_up_exp_sum as i64 {
                new_level = item.weapon_lv as u64;
                final_cum_sum = item.lv_up_exp_sum as i64;
            } else {
                break;
            }
        }

        let synced_exp = if new_level >= max_level {
            0
        } else {
            (new_total_exp - final_cum_sum).max(0)
        };
        (new_level, synced_exp as u64)
    } else {
        (current_level, current_exp)
    };

    if let Some(w) = ctx.player.char_bag.item_manager.weapons.get_mut(target_id) {
        w.weapon_lv = final_lv;
        w.exp = final_exp;
    }

    let updated_target = ctx
        .player
        .char_bag
        .item_manager
        .weapons
        .get(target_id)
        .map(|w| w.into());
    let del_inst_list: Vec<u64> = valid_fodders.iter().map(|id| id.as_u64()).collect();

    if updated_target.is_some() || !del_inst_list.is_empty() || !consumed_stackable.is_empty() {
        let mut depot = HashMap::new();
        depot.insert(
            1i32,
            ScdItemDepotModify {
                items: HashMap::new(),
                inst_list: updated_target.into_iter().collect(),
                del_inst_list,
            },
        );

        if !consumed_stackable.is_empty() {
            let items: HashMap<String, i64> = consumed_stackable
                .keys()
                .map(|id| {
                    let count = if ctx.player.char_bag.item_manager.has_stackable(
                        ItemDepotType::SpecialItem,
                        id,
                        0,
                    ) {
                        ctx.player
                            .char_bag
                            .item_manager
                            .count_of(ItemDepotType::SpecialItem, id)
                    } else {
                        ctx.player
                            .char_bag
                            .item_manager
                            .count_of(ItemDepotType::Factory, id)
                    };
                    (id.clone(), count as i64)
                })
                .collect();

            depot.insert(
                4i32,
                ScdItemDepotModify {
                    items,
                    inst_list: vec![],
                    del_inst_list: vec![],
                },
            );
        }

        let _ = ctx
            .notify(ScItemBagSyncModify {
                depot,
                bag: None,
                factory_depot: None,
                cannot_destroy: HashMap::new(),
                use_blackboard: None,
                is_new: false,
            })
            .await;
    }

    ScWeaponAddExp {
        weaponid: req.weaponid,
        new_exp: final_exp,
        weapon_lv: final_lv,
    }
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

    response.unwrap_or(ScWeaponDetachGem {
        weaponid: req.weaponid,
        detach_gemid: 0,
    })
}
