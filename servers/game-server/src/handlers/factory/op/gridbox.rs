//! GridBox + item-move ops.
//!
//! This is the biggest op family because it covers every combination of
//! source/destination for an item slot:
//!
//! - GridBoxInnerMove / GridBoxInnerSplit: rearrange items within one GridBox
//! - Bag <-> GridBox: move items between player bag and a storage box
//! - Depot <-> GridBox: move items between hub depot and a storage box
//! - Bag <-> Cache, Cache <-> Bag, Cache <-> Cache, Cache <-> Depot,
//!   Depot <-> Cache, Conveyor -> Bag: smaller item moves against
//!   crafter buffers / conveyor belts
//!
//! All of these need the inventory system, which isn't wired into the
//! factory yet. The handlers validate the request and find the target
//! component, then return `ok` with no actual item movement -- a
//! TODO marks each one.

use crate::net::NetContext;
use perlica_logic::factory::{FactoryComponent, GridBoxState, ItemSlot};
use perlica_proto::{
    CsdFactoryOpGridBoxInnerMove, CsdFactoryOpGridBoxInnerSplit, CsdFactoryOpMoveItemBagToCache,
    CsdFactoryOpMoveItemBagToGridBox, CsdFactoryOpMoveItemCacheToBag,
    CsdFactoryOpMoveItemCacheToCache, CsdFactoryOpMoveItemCacheToDepot,
    CsdFactoryOpMoveItemConveyorToBag, CsdFactoryOpMoveItemDepotToCache,
    CsdFactoryOpMoveItemDepotToGridBox, CsdFactoryOpMoveItemGridBoxToBag,
    CsdFactoryOpMoveItemGridBoxToDepot, FactoryOpRetCode, FactoryOpType, ScFactoryOpRet,
};

use super::super::response;

// ---- GridBox internal ops ----

pub async fn handle_inner_move(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpGridBoxInnerMove,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => return missing_region(index, FactoryOpType::GridBoxInnerMove, &region_name),
    };

    let state = match find_gridbox(region, req.component_id) {
        Some(s) => s,
        None => return missing_component(index, FactoryOpType::GridBoxInnerMove, req.component_id),
    };

    if req.from_index < 0 || req.to_index < 0 {
        return response::fail(
            index,
            FactoryOpType::GridBoxInnerMove,
            FactoryOpRetCode::Fail,
            "indices must be non-negative",
        );
    }
    let from = req.from_index as usize;
    let to = req.to_index as usize;

    if from >= state.items.len() {
        return response::fail(
            index,
            FactoryOpType::GridBoxInnerMove,
            FactoryOpRetCode::Fail,
            format!("from_index {} out of range", from),
        );
    }

    // Swap-or-move semantics: if the destination is empty, just move.
    // If it has the same item_id, stack. Otherwise swap.
    if to >= state.items.len() {
        // grow the slot list with empties up to `to`
        state.items.resize(to + 1, ItemSlot { item_id: String::new(), count: 0, inst_id: 0 });
        let moved = state.items[from].clone();
        state.items[to] = moved;
        state.items[from] = ItemSlot { item_id: String::new(), count: 0, inst_id: 0 };
    } else if state.items[to].item_id == state.items[from].item_id
        && state.items[to].inst_id == state.items[from].inst_id
        && !state.items[to].item_id.is_empty()
    {
        // same item, stack them
        state.items[to].count += state.items[from].count;
        state.items[from] = ItemSlot { item_id: String::new(), count: 0, inst_id: 0 };
    } else {
        // different items, swap. `split_at_mut` so we can borrow both
        // slots mutably at once without the borrow checker refusing.
        let (left, right) = state.items.split_at_mut(from.max(to));
        let (lower, higher) = if from < to {
            (&mut left[0], &mut right[0])
        } else {
            // from > to: to is in `left`, from is the first index of `right`
            (&mut left[to], &mut right[from - to - 1])
        };
        std::mem::swap(lower, higher);
    }

    let _ = ctx;
    response::ok(index, FactoryOpType::GridBoxInnerMove)
}

pub async fn handle_inner_split(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpGridBoxInnerSplit,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => return missing_region(index, FactoryOpType::GridBoxInnerSplit, &region_name),
    };

    let state = match find_gridbox(region, req.component_id) {
        Some(s) => s,
        None => return missing_component(index, FactoryOpType::GridBoxInnerSplit, req.component_id),
    };

    if req.from_index < 0 || req.to_index < 0 || req.count < 0 {
        return response::fail(
            index,
            FactoryOpType::GridBoxInnerSplit,
            FactoryOpRetCode::Fail,
            "indices and count must be non-negative",
        );
    }
    let from = req.from_index as usize;
    let to = req.to_index as usize;
    let count = req.count as u32;

    if from >= state.items.len() || state.items[from].count < count {
        return response::fail(
            index,
            FactoryOpType::GridBoxInnerSplit,
            FactoryOpRetCode::Fail,
            "source has insufficient items for split",
        );
    }

    if to >= state.items.len() {
        state.items.resize(to + 1, ItemSlot { item_id: String::new(), count: 0, inst_id: 0 });
    }

    // If destination is empty or matches, stack; otherwise reject so we
    // don't silently overwrite a different item.
    let same_dest = state.items[to].item_id == state.items[from].item_id
        && state.items[to].inst_id == state.items[from].inst_id;
    let empty_dest = state.items[to].item_id.is_empty();
    if !same_dest && !empty_dest {
        return response::fail(
            index,
            FactoryOpType::GridBoxInnerSplit,
            FactoryOpRetCode::Fail,
            "destination occupied by a different item",
        );
    }

    state.items[from].count -= count;
    state.items[to].item_id = state.items[from].item_id.clone();
    state.items[to].inst_id = state.items[from].inst_id;
    state.items[to].count += count;

    if state.items[from].count == 0 {
        state.items[from] = ItemSlot { item_id: String::new(), count: 0, inst_id: 0 };
    }

    let _ = ctx;
    response::ok(index, FactoryOpType::GridBoxInnerSplit)
}

// ---- Bag <-> GridBox ----

pub async fn handle_move_bag_to_gridbox(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemBagToGridBox,
) -> ScFactoryOpRet {
    // Validate the target GridBox exists, then leave the actual move for
    // the bag-integration TODO. The validation borrow is scoped so we
    // don't hold `&mut factory` while touching `ctx` for the noop.
    let found = {
        let region = match ctx.player.factory.region_mut(&region_name) {
            Some(r) => r,
            None => return missing_region(index, FactoryOpType::MoveItemBagToGridBox, &region_name),
        };
        find_gridbox(region, req.component_id).is_some()
    };
    if !found {
        return missing_component(index, FactoryOpType::MoveItemBagToGridBox, req.component_id);
    }

    // TODO(bag-integration): pull the item at `req.bag_grid_index` from
    // the player's bag, push it into the gridbox at `req.grid_box_index`.
    // Need the bag API exposed to factory handlers.
    let _ = (req.bag_grid_index, req.grid_box_index);
    response::ok(index, FactoryOpType::MoveItemBagToGridBox)
}

pub async fn handle_move_gridbox_to_bag(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemGridBoxToBag,
) -> ScFactoryOpRet {
    let found = {
        let region = match ctx.player.factory.region_mut(&region_name) {
            Some(r) => r,
            None => return missing_region(index, FactoryOpType::MoveItemGridBoxToBag, &region_name),
        };
        find_gridbox(region, req.component_id).is_some()
    };
    if !found {
        return missing_component(index, FactoryOpType::MoveItemGridBoxToBag, req.component_id);
    }

    // TODO(bag-integration): take the slot at `req.grid_box_index`,
    // push it into the bag at `req.bag_grid_index`, clear the gridbox slot.
    let _ = (req.grid_box_index, req.bag_grid_index);
    response::ok(index, FactoryOpType::MoveItemGridBoxToBag)
}

// ---- Depot <-> GridBox ----

pub async fn handle_move_depot_to_gridbox(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemDepotToGridBox,
) -> ScFactoryOpRet {
    let found = {
        let region = match ctx.player.factory.region_mut(&region_name) {
            Some(r) => r,
            None => return missing_region(index, FactoryOpType::MoveItemDepotToGridBox, &region_name),
        };
        find_gridbox(region, req.component_id).is_some()
    };
    if !found {
        return missing_component(index, FactoryOpType::MoveItemDepotToGridBox, req.component_id);
    }

    // TODO(depot-integration): pull `req.item_id` from the hub depot
    // (node_id=2, Inventory component id=8), push into the gridbox at
    // `req.grid_box_index`.
    let _ = (req.item_id, req.grid_box_index);
    response::ok(index, FactoryOpType::MoveItemDepotToGridBox)
}

pub async fn handle_move_gridbox_to_depot(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemGridBoxToDepot,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => return missing_region(index, FactoryOpType::MoveItemGridBoxToDepot, &region_name),
    };
    let state = match find_gridbox(region, req.component_id) {
        Some(s) => s,
        None => return missing_component(index, FactoryOpType::MoveItemGridBoxToDepot, req.component_id),
    };

    if req.grid_box_index < 0 || req.grid_box_index as usize >= state.items.len() {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToDepot,
            FactoryOpRetCode::Fail,
            "grid_box_index out of range",
        );
    }

    // TODO(depot-integration): pop `state.items[req.grid_box_index]`,
    // push into the hub depot's inventory. Clear the gridbox slot.
    let _ = ctx;
    response::ok(index, FactoryOpType::MoveItemGridBoxToDepot)
}

// ---- Cache ops ----

pub async fn handle_move_cache_to_cache(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemCacheToCache,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => return missing_region(index, FactoryOpType::MoveItemCacheToCache, &region_name),
    };

    // Both component IDs must exist and be Cache components.
    let mut from_state: Option<Vec<ItemSlot>> = None;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.from_component_id) {
            if let FactoryComponent::Cache(state) = slot {
                from_state = Some(std::mem::take(&mut state.items));
                break;
            }
        }
    }
    let from_items = match from_state {
        Some(v) => v,
        None => {
            return missing_component(index, FactoryOpType::MoveItemCacheToCache, req.from_component_id);
        }
    };

    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.to_component_id) {
            if let FactoryComponent::Cache(state) = slot {
                move_item_into(&mut state.items, &req.item_id, from_items);
                let _ = ctx;
                return response::ok(index, FactoryOpType::MoveItemCacheToCache);
            }
        }
    }

    missing_component(index, FactoryOpType::MoveItemCacheToCache, req.to_component_id)
}

pub async fn handle_move_bag_to_cache(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemBagToCache,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => return missing_region(index, FactoryOpType::MoveItemBagToCache, &region_name),
    };

    let mut found = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id) {
            if let FactoryComponent::Cache(_) = slot {
                found = true;
                break;
            }
        }
    }
    if !found {
        return missing_component(index, FactoryOpType::MoveItemBagToCache, req.component_id);
    }

    // TODO(bag-integration): pull item at `req.grid_index` from bag,
    // push into the matched Cache.
    let _ = (ctx, req.grid_index);
    response::ok(index, FactoryOpType::MoveItemBagToCache)
}

pub async fn handle_move_cache_to_bag(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemCacheToBag,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => return missing_region(index, FactoryOpType::MoveItemCacheToBag, &region_name),
    };

    let mut found_item = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id) {
            if let FactoryComponent::Cache(state) = slot {
                // Pull the requested item out of the cache if we have it.
                let was_present = state
                    .items
                    .iter()
                    .any(|s| s.item_id == req.item_id && s.count > 0);
                if was_present {
                    if let Some(slot) = state
                        .items
                        .iter_mut()
                        .find(|s| s.item_id == req.item_id && s.count > 0)
                    {
                        slot.count = slot.count.saturating_sub(1);
                        found_item = true;
                    }
                }
                break;
            }
        }
    }

    if !found_item {
        // We didn't have the item; that's fine, just report ok with no
        // movement. The bag integration TODO below covers actually
        // adding the item to the bag.
    }

    // TODO(bag-integration): push the pulled item into the player's bag.
    let _ = ctx;
    response::ok(index, FactoryOpType::MoveItemCacheToBag)
}

pub async fn handle_move_depot_to_cache(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemDepotToCache,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => return missing_region(index, FactoryOpType::MoveItemDepotToCache, &region_name),
    };

    let mut found = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id) {
            if let FactoryComponent::Cache(state) = slot {
                state.items.push(ItemSlot {
                    item_id: req.item_id.clone(),
                    count: 1,
                    inst_id: 0,
                });
                found = true;
                break;
            }
        }
    }
    if !found {
        return missing_component(index, FactoryOpType::MoveItemDepotToCache, req.component_id);
    }

    // TODO(depot-integration): actually deduct the item from the hub
    // depot's inventory. Above we blindly pushed into the cache without
    // checking the depot has it, which is wrong.
    let _ = ctx;
    response::ok(index, FactoryOpType::MoveItemDepotToCache)
}

pub async fn handle_move_cache_to_depot(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemCacheToDepot,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => return missing_region(index, FactoryOpType::MoveItemCacheToDepot, &region_name),
    };

    let mut moved = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id) {
            if let FactoryComponent::Cache(state) = slot {
                if let Some(s) = state
                    .items
                    .iter_mut()
                    .find(|s| s.item_id == req.item_id && s.count > 0)
                {
                    s.count = s.count.saturating_sub(1);
                    moved = true;
                }
                break;
            }
        }
    }

    if !moved {
        // Nothing to move; still ok so the client doesn't error out.
        let _ = ctx;
        return response::ok(index, FactoryOpType::MoveItemCacheToDepot);
    }

    // TODO(depot-integration): add the pulled item to the hub depot.
    let _ = ctx;
    response::ok(index, FactoryOpType::MoveItemCacheToDepot)
}

pub async fn handle_move_conveyor_to_bag(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemConveyorToBag,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => return missing_region(index, FactoryOpType::MoveItemConveyorToBag, &region_name),
    };

    let mut found = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id) {
            if let FactoryComponent::BoxConveyor(state) = slot {
                if req.all {
                    state.items.clear();
                } else if req.index >= 0 && (req.index as usize) < state.items.len() {
                    state.items.remove(req.index as usize);
                }
                found = true;
                break;
            }
        }
    }
    if !found {
        return missing_component(index, FactoryOpType::MoveItemConveyorToBag, req.component_id);
    }

    // TODO(bag-integration): push the removed item(s) into the bag.
    let _ = ctx;
    response::ok(index, FactoryOpType::MoveItemConveyorToBag)
}

// ---- helpers ----

fn find_gridbox<'a>(
    region: &'a mut perlica_logic::factory::FactoryRegion,
    component_id: u32,
) -> Option<&'a mut GridBoxState> {
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(component_id) {
            if let FactoryComponent::GridBox(state) = slot {
                return Some(state);
            }
            // wrong component type -- not found.
            return None;
        }
    }
    None
}

fn move_item_into(dest: &mut Vec<ItemSlot>, item_id: &str, source: Vec<ItemSlot>) {
    // Pull matching slots from the source and stack them into the
    // destination, then leave the leftovers in `source`. Caller is
    // expected to write the leftover back if needed (we don't here --
    // this is a one-way move).
    let _ = source; // source items would normally be drained here
    if let Some(slot) = dest.iter_mut().find(|s| s.item_id == item_id) {
        slot.count += 1;
    } else {
        dest.push(ItemSlot {
            item_id: item_id.to_string(),
            count: 1,
            inst_id: 0,
        });
    }
}

fn missing_region(index: String, op_type: FactoryOpType, region: &str) -> ScFactoryOpRet {
    response::fail(
        index,
        op_type,
        FactoryOpRetCode::Fail,
        format!("region {} not found", region),
    )
}

fn missing_component(index: String, op_type: FactoryOpType, cid: u32) -> ScFactoryOpRet {
    response::fail(
        index,
        op_type,
        FactoryOpRetCode::Fail,
        format!("component {} not found", cid),
    )
}
