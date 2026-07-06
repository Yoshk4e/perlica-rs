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
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::GridBoxInnerMove, &region_name);
    };

    let Some(state) = find_gridbox(region, req.component_id) else {
        return missing_component(index, FactoryOpType::GridBoxInnerMove, req.component_id);
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
            format!("from_index {from} out of range"),
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
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::GridBoxInnerSplit, &region_name);
    };

    let Some(state) = find_gridbox(region, req.component_id) else {
        return missing_component(index, FactoryOpType::GridBoxInnerSplit, req.component_id);
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
    let (src_item_id, src_inst_id) = (
        state.items[from].item_id.clone(),
        state.items[from].inst_id,
    );
    state.items[to].item_id = src_item_id;
    state.items[to].inst_id = src_inst_id;
    state.items[to].count += count;

    if state.items[from].count == 0 {
        state.items[from] = ItemSlot { item_id: String::new(), count: 0, inst_id: 0 };
    }

    let _ = ctx;
    response::ok(index, FactoryOpType::GridBoxInnerSplit)
}

// ---- Bag <-> GridBox ----
//
// The player's bag is represented as the inventory node (node_id=1) with
// its Inventory component. Items move between the bag and a GridBox one
// stack at a time. `bag_grid_index` is the slot in the bag, `grid_box_index`
// is the slot in the GridBox.

pub async fn handle_move_bag_to_gridbox(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemBagToGridBox,
) -> ScFactoryOpRet {
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::MoveItemBagToGridBox, &region_name);
    };

    // Pull the item from the bag (inventory node, component_id=1).
    let bag_item = {
        let Some(inv_node) = region.node_mut(1) else {
            return response::fail(
                index,
                FactoryOpType::MoveItemBagToGridBox,
                FactoryOpRetCode::Fail,
                "inventory node not found",
            );
        };
        let Some(slot) = inv_node.component_mut(1) else {
            return response::fail(
                index,
                FactoryOpType::MoveItemBagToGridBox,
                FactoryOpRetCode::Fail,
                "inventory component not found",
            );
        };
        let FactoryComponent::Inventory(inv_state) = slot else {
            return response::fail(
                index,
                FactoryOpType::MoveItemBagToGridBox,
                FactoryOpRetCode::Fail,
                "component 1 is not an Inventory",
            );
        };
        // The bag uses inst_id as the key. bag_grid_index maps to a
        // specific inst_id -- since we don't have a slot-indexed bag
        // yet, treat bag_grid_index as the inst_id directly.
        let key = req.bag_grid_index as u32;
        inv_state.items.remove(&key)
    };

    let Some(item) = bag_item else {
        return response::fail(
            index,
            FactoryOpType::MoveItemBagToGridBox,
            FactoryOpRetCode::Fail,
            "bag slot is empty",
        );
    };

    // Push into the GridBox at the requested index.
    let Some(gridbox) = find_gridbox(region, req.component_id) else {
        return missing_component(index, FactoryOpType::MoveItemBagToGridBox, req.component_id);
    };

    let idx = req.grid_box_index as usize;
    if idx >= gridbox.items.len() {
        gridbox.items.resize(idx + 1, ItemSlot { item_id: String::new(), count: 0, inst_id: 0 });
    }

    // Stack if same item, otherwise overwrite (the client guarantees the
    // destination is empty or matches).
    if gridbox.items[idx].item_id == item.item_id && gridbox.items[idx].count > 0 {
        gridbox.items[idx].count += item.count;
    } else {
        gridbox.items[idx] = item;
    }

    response::ok(index, FactoryOpType::MoveItemBagToGridBox)
}

pub async fn handle_move_gridbox_to_bag(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemGridBoxToBag,
) -> ScFactoryOpRet {
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::MoveItemGridBoxToBag, &region_name);
    };

    let Some(gridbox) = find_gridbox(region, req.component_id) else {
        return missing_component(index, FactoryOpType::MoveItemGridBoxToBag, req.component_id);
    };

    if req.grid_box_index < 0 || req.grid_box_index as usize >= gridbox.items.len() {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToBag,
            FactoryOpRetCode::Fail,
            "grid_box_index out of range",
        );
    }

    // Pop the item from the GridBox slot.
    let item = std::mem::replace(
        &mut gridbox.items[req.grid_box_index as usize],
        ItemSlot { item_id: String::new(), count: 0, inst_id: 0 },
    );

    if item.count == 0 {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToBag,
            FactoryOpRetCode::Fail,
            "gridbox slot is empty",
        );
    }

    // Push into the bag at bag_grid_index (used as inst_id key).
    let Some(inv_node) = region.node_mut(1) else {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToBag,
            FactoryOpRetCode::Fail,
            "inventory node not found",
        );
    };
    let Some(slot) = inv_node.component_mut(1) else {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToBag,
            FactoryOpRetCode::Fail,
            "inventory component not found",
        );
    };
    let FactoryComponent::Inventory(inv_state) = slot else {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToBag,
            FactoryOpRetCode::Fail,
            "component 1 is not an Inventory",
        );
    };
    inv_state.items.insert(req.bag_grid_index as u32, item);

    response::ok(index, FactoryOpType::MoveItemGridBoxToBag)
}

// ---- Depot <-> GridBox ----
//
// The depot is the hub's inventory (node_id=2, component_id=8). Items
// move between the depot and a GridBox by item_id (depot side) and
// grid_box_index (GridBox side).

pub async fn handle_move_depot_to_gridbox(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemDepotToGridBox,
) -> ScFactoryOpRet {
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::MoveItemDepotToGridBox, &region_name);
    };

    // Pull one unit of the requested item from the depot.
    let depot_item = {
        let Some(hub_node) = region.node_mut(2) else {
            return response::fail(
                index,
                FactoryOpType::MoveItemDepotToGridBox,
                FactoryOpRetCode::Fail,
                "hub node not found",
            );
        };
        let Some(slot) = hub_node.component_mut(8) else {
            return response::fail(
                index,
                FactoryOpType::MoveItemDepotToGridBox,
                FactoryOpRetCode::Fail,
                "depot component not found",
            );
        };
        let FactoryComponent::Inventory(inv_state) = slot else {
            return response::fail(
                index,
                FactoryOpType::MoveItemDepotToGridBox,
                FactoryOpRetCode::Fail,
                "component 8 is not an Inventory",
            );
        };
        // Find the item by item_id (stackables use inst_id=0) and
        // decrement its count by 1.
        let mut found = None;
        for (&inst_id, slot) in &mut inv_state.items {
            if slot.item_id == req.item_id && slot.count > 0 {
                slot.count -= 1;
                found = Some(ItemSlot {
                    item_id: req.item_id.clone(),
                    count: 1,
                    inst_id,
                });
                if slot.count == 0 {
                    inv_state.items.remove(&inst_id);
                }
                break;
            }
        }
        found
    };

    let Some(item) = depot_item else {
        return response::fail(
            index,
            FactoryOpType::MoveItemDepotToGridBox,
            FactoryOpRetCode::Fail,
            "depot doesn't have that item",
        );
    };

    // Push into the GridBox.
    let Some(gridbox) = find_gridbox(region, req.component_id) else {
        return missing_component(index, FactoryOpType::MoveItemDepotToGridBox, req.component_id);
    };

    let idx = req.grid_box_index as usize;
    if idx >= gridbox.items.len() {
        gridbox.items.resize(idx + 1, ItemSlot { item_id: String::new(), count: 0, inst_id: 0 });
    }

    if gridbox.items[idx].item_id == item.item_id && gridbox.items[idx].count > 0 {
        gridbox.items[idx].count += item.count;
    } else {
        gridbox.items[idx] = item;
    }

    response::ok(index, FactoryOpType::MoveItemDepotToGridBox)
}

pub async fn handle_move_gridbox_to_depot(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemGridBoxToDepot,
) -> ScFactoryOpRet {
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::MoveItemGridBoxToDepot, &region_name);
    };

    let Some(gridbox) = find_gridbox(region, req.component_id) else {
        return missing_component(index, FactoryOpType::MoveItemGridBoxToDepot, req.component_id);
    };

    if req.grid_box_index < 0 || req.grid_box_index as usize >= gridbox.items.len() {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToDepot,
            FactoryOpRetCode::Fail,
            "grid_box_index out of range",
        );
    }

    // Pop the entire stack from the GridBox slot.
    let item = std::mem::replace(
        &mut gridbox.items[req.grid_box_index as usize],
        ItemSlot { item_id: String::new(), count: 0, inst_id: 0 },
    );

    if item.count == 0 {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToDepot,
            FactoryOpRetCode::Fail,
            "gridbox slot is empty",
        );
    }

    // Push into the depot inventory. Stack with existing same-item slots.
    let Some(hub_node) = region.node_mut(2) else {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToDepot,
            FactoryOpRetCode::Fail,
            "hub node not found",
        );
    };
    let Some(slot) = hub_node.component_mut(8) else {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToDepot,
            FactoryOpRetCode::Fail,
            "depot component not found",
        );
    };
    let FactoryComponent::Inventory(inv_state) = slot else {
        return response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToDepot,
            FactoryOpRetCode::Fail,
            "component 8 is not an Inventory",
        );
    };

    // Try to stack with an existing slot of the same item_id.
    let mut stacked = false;
    for existing in inv_state.items.values_mut() {
        if existing.item_id == item.item_id && existing.inst_id == item.inst_id {
            existing.count += item.count;
            stacked = true;
            break;
        }
    }
    if !stacked {
        inv_state.items.insert(item.inst_id, item);
    }

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
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::MoveItemCacheToCache, &region_name);
    };

    // Both component IDs must exist and be Cache components.
    let mut from_state: Option<Vec<ItemSlot>> = None;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.from_component_id)
            && let FactoryComponent::Cache(state) = slot {
                from_state = Some(std::mem::take(&mut state.items));
                break;
            }
    }
    let Some(from_items) = from_state else {
            return missing_component(index, FactoryOpType::MoveItemCacheToCache, req.from_component_id);
    };

    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.to_component_id)
            && let FactoryComponent::Cache(state) = slot {
                move_item_into(&mut state.items, &req.item_id, from_items);
                let _ = ctx;
                return response::ok(index, FactoryOpType::MoveItemCacheToCache);
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
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::MoveItemBagToCache, &region_name);
    };

    let mut found = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id)
            && let FactoryComponent::Cache(_) = slot {
                found = true;
                break;
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
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::MoveItemCacheToBag, &region_name);
    };

    let mut found_item = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id)
            && let FactoryComponent::Cache(state) = slot {
                // Pull the requested item out of the cache if we have it.
                let was_present = state
                    .items
                    .iter()
                    .any(|s| s.item_id == req.item_id && s.count > 0);
                if was_present
                    && let Some(slot) = state
                        .items
                        .iter_mut()
                        .find(|s| s.item_id == req.item_id && s.count > 0)
                    {
                        slot.count = slot.count.saturating_sub(1);
                        found_item = true;
                    }
                break;
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
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::MoveItemDepotToCache, &region_name);
    };

    let mut found = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id)
            && let FactoryComponent::Cache(state) = slot {
                state.items.push(ItemSlot {
                    item_id: req.item_id.clone(),
                    count: 1,
                    inst_id: 0,
                });
                found = true;
                break;
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
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::MoveItemCacheToDepot, &region_name);
    };

    let mut moved = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id)
            && let FactoryComponent::Cache(state) = slot {
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
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
        return missing_region(index, FactoryOpType::MoveItemConveyorToBag, &region_name);
    };

    let mut found = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id)
            && let FactoryComponent::BoxConveyor(state) = slot {
                if req.all {
                    state.items.clear();
                } else if req.index >= 0 && (req.index as usize) < state.items.len() {
                    state.items.remove(req.index as usize);
                }
                found = true;
                break;
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

fn find_gridbox(
    region: &mut perlica_logic::factory::FactoryRegion,
    component_id: u32,
) -> Option<&mut GridBoxState> {
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
        format!("region {region} not found"),
    )
}

fn missing_component(index: String, op_type: FactoryOpType, cid: u32) -> ScFactoryOpRet {
    response::fail(
        index,
        op_type,
        FactoryOpRetCode::Fail,
        format!("component {cid} not found"),
    )
}
