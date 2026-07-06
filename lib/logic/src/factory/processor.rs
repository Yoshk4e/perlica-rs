//! Processor crafting business logic.
//!
//! The processor is a per-region building that crafts items, equipment,
//! and gems from recipes in `processorCraftData`. Higher-tier crafts
//! consume refine points, which regenerate over time (1 per
//! `refine_point_recover_time` ticks, up to `refine_point_max`).
//!
//! Gem recast requires building level >= `building_level_gem_recast` (3).
//! Weapon refine requires building level >= `building_level_weapon_refine` (4).
//!
//! This module owns the game logic -- ingredient consumption, refine
//! point recovery, level gating. The handler in `game-server` is just
//! a thin wrapper that decodes the proto, calls these functions, and
//! encodes the response.

use config::factory_table::FTableAssets;
use config::factory_processor_const::FProcessorConstAssets;

use crate::factory::{FactoryComponent, FactoryManager, InventoryState, ProcessorMachine, Tick, current_tick};

/// Result of a processor craft attempt. On success, `new_items` holds
/// the produced items. On failure, it's empty and the caller should
/// just send an empty response.
#[derive(Debug, Clone)]
pub struct CraftResult {
    pub new_items: Vec<CraftedItem>,
}

#[derive(Debug, Clone)]
pub struct CraftedItem {
    pub item_id: String,
    pub count: u32,
}

/// Make a regular item via the processor. No refine point cost, just
/// ingredients from the player's bag.
pub fn make_item(
    manager: &mut FactoryManager,
    assets: &FTableAssets,
    region_name: &str,
    formula_id: &str,
    count: u32,
) -> CraftResult {
    let Some(recipe) = assets.get_processor_craft(formula_id).cloned() else {
        return CraftResult { new_items: vec![] };
    };

    if !consume_ingredients(manager, region_name, &recipe.ingredients, count) {
        return CraftResult { new_items: vec![] };
    }

    CraftResult {
        new_items: vec![CraftedItem {
            item_id: recipe.outcome.id.clone(),
            count: recipe.outcome.count.saturating_mul(count),
        }],
    }
}

/// Make an equipment via the processor. Optionally consumes a refine
/// point if `use_refine_point` is set. Checks building level for
/// weapon refine.
pub fn make_equip(
    manager: &mut FactoryManager,
    assets: &FTableAssets,
    proc_const: &FProcessorConstAssets,
    region_name: &str,
    formula_id: &str,
    count: u32,
    use_refine_point: bool,
) -> CraftResult {
    let Some(recipe) = assets.get_processor_craft(formula_id).cloned() else {
        return CraftResult { new_items: vec![] };
    };

    let proc_level = get_processor_level(manager, region_name);

    if use_refine_point {
        let weapon_refine_level = proc_const.data.building_level_weapon_refine;
        if proc_level < weapon_refine_level {
            return CraftResult { new_items: vec![] };
        }

        let recover_time = proc_const.data.refine_point_recover_time;
        let max_points = proc_const.data.refine_point_max;
        let Some(proc) = get_or_create_processor(manager, region_name, max_points) else {
            return CraftResult { new_items: vec![] };
        };
        recover_refine_points(proc, recover_time, max_points);
        if proc.refine_points == 0 {
            return CraftResult { new_items: vec![] };
        }
        proc.refine_points -= 1;
    }

    if !consume_ingredients(manager, region_name, &recipe.ingredients, count) {
        return CraftResult { new_items: vec![] };
    }

    CraftResult {
        new_items: vec![CraftedItem {
            item_id: recipe.outcome.id.clone(),
            count: recipe.outcome.count.saturating_mul(count),
        }],
    }
}

/// Make a gem via the processor. Consumes the gem instances listed in
/// `cost_gem_inst_ids` from the player's bag.
pub fn make_gem(
    manager: &mut FactoryManager,
    assets: &FTableAssets,
    region_name: &str,
    formula_id: &str,
    count: u32,
    cost_gem_inst_ids: &[u64],
) -> CraftResult {
    let Some(recipe) = assets.get_processor_craft(formula_id).cloned() else {
        return CraftResult { new_items: vec![] };
    };

    remove_gems_from_bag(manager, region_name, cost_gem_inst_ids);

    CraftResult {
        new_items: vec![CraftedItem {
            item_id: recipe.outcome.id.clone(),
            count: recipe.outcome.count.saturating_mul(count),
        }],
    }
}

/// Recast a gem. Requires building level >= `building_level_gem_recast`.
/// Same consumption logic as `make_gem`.
pub fn recast_gem(
    manager: &mut FactoryManager,
    assets: &FTableAssets,
    proc_const: &FProcessorConstAssets,
    region_name: &str,
    formula_id: &str,
    count: u32,
    cost_gem_inst_ids: &[u64],
) -> CraftResult {
    let proc_level = get_processor_level(manager, region_name);
    let gem_recast_level = proc_const.data.building_level_gem_recast;
    if proc_level < gem_recast_level {
        return CraftResult { new_items: vec![] };
    }

    make_gem(manager, assets, region_name, formula_id, count, cost_gem_inst_ids)
}

/// Mark formula IDs as read across all processor states. Moves them
/// out of `unread_formulas`.
pub fn mark_formulas_read(manager: &mut FactoryManager, read_formula_ids: &[String]) {
    for proc in manager.processor_state.values_mut() {
        proc.unread_formulas
            .retain(|f| !read_formula_ids.contains(f));
    }
}

// ---- internal helpers ----

/// Walk the player's bag (inventory node) and consume `count` units of
/// each ingredient. Returns false if any ingredient is insufficient.
fn consume_ingredients(
    manager: &mut FactoryManager,
    region_name: &str,
    ingredients: &[config::tables::factory_table::ItemCount],
    count: u32,
) -> bool {
    let Some(region) = manager.region_mut(region_name) else {
        return false;
    };
    let Some(bag_node) = region.node_mut(1) else {
        return false;
    };
    let Some(bag_comp) = bag_node.component_mut(1) else {
        return false;
    };
    let FactoryComponent::Inventory(bag_inv) = bag_comp else {
        return false;
    };

    for ingredient in ingredients {
        let needed = ingredient.count.saturating_mul(count);
        if !try_consume_item(bag_inv, &ingredient.id, needed) {
            return false;
        }
    }
    true
}

/// Try to consume `needed` units of `item_id` from the inventory.
/// Walks slots, decrements counts, removes empty slots. Returns false
/// if there isn't enough.
fn try_consume_item(inv: &mut InventoryState, item_id: &str, needed: u32) -> bool {
    let mut remaining = needed;
    let mut to_remove = vec![];

    for (&inst_id, slot) in &mut inv.items {
        if slot.item_id == item_id && slot.count > 0 {
            let take = slot.count.min(remaining);
            slot.count -= take;
            remaining -= take;
            if slot.count == 0 {
                to_remove.push(inst_id);
            }
            if remaining == 0 {
                break;
            }
        }
    }

    for inst_id in to_remove {
        inv.items.remove(&inst_id);
    }

    remaining == 0
}

/// Remove gem instances by inst_id from the bag.
fn remove_gems_from_bag(manager: &mut FactoryManager, region_name: &str, inst_ids: &[u64]) {
    let Some(region) = manager.region_mut(region_name) else {
        return;
    };
    let Some(bag_node) = region.node_mut(1) else {
        return;
    };
    let Some(bag_comp) = bag_node.component_mut(1) else {
        return;
    };
    let FactoryComponent::Inventory(bag_inv) = bag_comp else {
        return;
    };

    for &inst_id in inst_ids {
        bag_inv.items.remove(&(inst_id as u32));
    }
}

/// Get the building level of the processor in a region. Returns 0 if
/// no processor state exists.
fn get_processor_level(manager: &FactoryManager, region_name: &str) -> u32 {
    manager
        .processor_state
        .get(region_name)
        .map_or(0, |p| p.building_level)
}

/// Get or create processor state for a region. New state starts with
/// full refine points.
fn get_or_create_processor<'a>(
    manager: &'a mut FactoryManager,
    region_name: &str,
    max_points: u32,
) -> Option<&'a mut ProcessorMachine> {
    let now: Tick = current_tick();
    let proc = manager
        .processor_state
        .entry(region_name.to_string())
        .or_insert(ProcessorMachine {
            region_name: region_name.to_string(),
            building_level: 1,
            refine_points: max_points,
            last_recovery_tick: now,
            unlocked_formulas: vec![],
            unread_formulas: vec![],
        });
    Some(proc)
}

/// Lazily recover refine points. Each `recover_time` ticks = 1 point,
/// up to `max_points`.
fn recover_refine_points(proc: &mut ProcessorMachine, recover_time: u64, max_points: u32) {
    let now: Tick = current_tick();
    let elapsed = now.saturating_sub(proc.last_recovery_tick);
    if recover_time == 0 {
        return;
    }
    let recovered = (elapsed / recover_time) as u32;
    if recovered > 0 {
        proc.refine_points = (proc.refine_points + recovered).min(max_points);
        proc.last_recovery_tick = now;
    }
}
