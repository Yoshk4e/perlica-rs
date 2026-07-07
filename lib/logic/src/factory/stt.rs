//! STT tech tree business logic.
//!
//! The tech tree has 31 regular nodes + 1 special. Each node has
//! conditions (type 504 = "have N of building X", type 511 = "produce
//! N total items") and an action (type 501 = "give blueprint items").
//! When a player tries to unlock a node, the server checks all
//! conditions, deducts cost items, grants unlock rewards, and fires
//! the action.

use config::factory_sttree::FSttreeAssets;
use config::factory_table::FTableAssets;

use crate::factory::{FactoryComponent, FactoryManager, ItemSlot};

impl FactoryManager {
    /// Try to unlock an STT node. Checks conditions, deducts cost items
    /// from the bag, grants rewards, and fires the node's action.
    /// Returns true on success.
    pub fn stt_unlock_node(
        &mut self,
        sttree: &FSttreeAssets,
        _assets: &FTableAssets,
        region_name: &str,
        node_id: &str,
    ) -> bool {
        // Already unlocked?
        if self.stt_state.unlocked_nodes.contains(&node_id.to_string()) {
            return false;
        }

        let Some(node) = sttree.get_node(node_id) else {
            return false;
        };

        // Check all conditions.
        let Some(region) = self.region(region_name) else {
            return false;
        };
        for condition in &node.conditions {
            if !evaluate_condition(condition, region) {
                return false;
            }
        }

        // Check prerequisite nodes.
        for pre in &node.pre_node {
            if !pre.is_empty() && !self.stt_state.unlocked_nodes.contains(pre) {
                return false;
            }
        }

        // Deduct cost items from the bag.
        let Some(region) = self.region_mut(region_name) else {
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

        // Verify + consume cost items.
        for cost in &node.cost_items {
            if !try_consume_from_inv(bag_inv, &cost.id, cost.count) {
                return false;
            }
        }

        // Grant unlock rewards.
        for reward in &node.unlock_reward {
            let mut stacked = false;
            for slot in bag_inv.items.values_mut() {
                if slot.item_id == reward.item_id {
                    slot.count += reward.count;
                    stacked = true;
                    break;
                }
            }
            if !stacked {
                bag_inv.items.insert(
                    0,
                    ItemSlot {
                        item_id: reward.item_id.clone(),
                        count: reward.count,
                        inst_id: 0,
                    },
                );
            }
        }

        // Fire the node's action (type 501 = give blueprint items).
        if let Some(action) = &node.action {
            execute_action(action, bag_inv);
        }

        // Mark as unlocked.
        self.stt_state.unlocked_nodes.push(node_id.to_string());

        true
    }
}

/// Evaluate a single STT condition against a region's state.
fn evaluate_condition(
    condition: &config::tables::factory_sttree::FacSttConditionEntry,
    region: &crate::factory::FactoryRegion,
) -> bool {
    match condition.condition_type {
        // Type 504: "Have N of building X placed"
        504 => {
            let building_id = condition
                .parameters
                .first()
                .and_then(|p| p.value_string_list.as_ref())
                .and_then(|l| l.first())
                .map_or("", String::as_str);
            let required = condition
                .parameters
                .get(1)
                .and_then(|p| p.value_int_list.as_ref())
                .and_then(|l| l.first())
                .copied()
                .unwrap_or(0) as usize;
            !building_id.is_empty() && region.count_buildings_by_template(building_id) >= required
        }
        // Type 511: "Produce N total items"
        511 => {
            let required = condition
                .parameters
                .first()
                .and_then(|p| p.value_int_list.as_ref())
                .and_then(|l| l.first())
                .copied()
                .unwrap_or(0) as u64;
            let total: u64 = region.production_totals.values().sum();
            total >= required
        }
        _ => false,
    }
}

/// Execute an STT action. Type 501 = give blueprint items to the bag.
fn execute_action(
    action: &config::tables::factory_sttree::FacSttActionEntry,
    inv: &mut crate::factory::InventoryState,
) {
    if action.action_type == 501 {
        // Give blueprint items. Parameters[0] = item_id, [1] = count.
        let item_id = action
            .parameters
            .first()
            .and_then(|p| p.value_string_list.as_ref())
            .and_then(|l| l.first())
            .cloned()
            .unwrap_or_default();
        let count = action
            .parameters
            .get(1)
            .and_then(|p| p.value_int_list.as_ref())
            .and_then(|l| l.first())
            .copied()
            .unwrap_or(1) as u32;

        if !item_id.is_empty() {
            let mut stacked = false;
            for slot in inv.items.values_mut() {
                if slot.item_id == item_id {
                    slot.count += count;
                    stacked = true;
                    break;
                }
            }
            if !stacked {
                inv.items.insert(
                    0,
                    ItemSlot {
                        item_id,
                        count,
                        inst_id: 0,
                    },
                );
            }
        }
    }
}

fn try_consume_from_inv(
    inv: &mut crate::factory::InventoryState,
    item_id: &str,
    needed: u32,
) -> bool {
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
