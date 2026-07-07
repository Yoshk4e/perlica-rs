//! Trade / contracts business logic.
//!
//! The trader generates orders from the player's active contract using
//! weighted-random selection (`orderIdGroup` + `orderWeight`). Orders
//! generate every `tradeOrderGenSpeed = 360` ticks. Players commit
//! items to fill order value (item values come from the contract's
//! `itemValueGroup`), and when `accumulated_value >= minValue` the
//! order is "filled" and the player receives rewards from `rewardItems`.
//!
//! Three ops: `set_contract` (pick which contract is active),
//! `cash_order` (commit items to fill an order), `delete_order`
//! (cancel an order).

use config::contract_table::ContractAssets;
use config::factory_table::FTableAssets;

use crate::factory::{FactoryManager, ItemSlot, TradeOrder, current_tick};

impl FactoryManager {
    /// Set the active contract for a trader. Replaces any previously
    /// active contract and clears pending orders.
    pub fn trade_set_contract(
        &mut self,
        contracts: &ContractAssets,
        region_name: &str,
        _node_id: u32,
        contract_id: &str,
    ) -> bool {
        if contracts.get_contract(contract_id).is_none() {
            return false;
        }

        let trade = self.trade_state.entry(region_name.to_string()).or_insert(
            crate::factory::TradeMachine {
                region_name: region_name.to_string(),
                building_level: 1,
                active_contract: None,
                orders: vec![],
                last_gen_tick: current_tick(),
            },
        );

        trade.active_contract = Some(contract_id.to_string());
        trade.orders.clear();
        trade.last_gen_tick = current_tick();
        true
    }

    /// Commit items to a trade order. Items are consumed from the bag,
    /// their value (from the contract's `itemValueGroup`) is added to
    /// the order's `accumulated_value`. When the order reaches
    /// `minValue`, it's considered filled and rewards are granted.
    pub fn trade_cash_order(
        &mut self,
        assets: &FTableAssets,
        contracts: &ContractAssets,
        region_name: &str,
        _node_id: u32,
        inst_id: u64,
        items: &std::collections::HashMap<String, i64>,
    ) -> bool {
        // First, generate any pending orders (lazy tick).
        self.trade_generate_orders(assets, contracts, region_name);

        // Look up contract + order index without holding a region borrow.
        let (contract_min_value, contract_item_values) = {
            let Some(trade) = self.trade_state.get(region_name) else {
                return false;
            };
            let Some(contract_id) = &trade.active_contract else {
                return false;
            };
            let Some(contract) = contracts.get_contract(contract_id) else {
                return false;
            };
            let Some(_idx) = trade
                .orders
                .iter()
                .position(|o| o.inst_id == inst_id as u32)
            else {
                return false;
            };
            // Collect the values we need so we don't hold the borrow.
            let values: Vec<(String, u64)> = contract
                .item_value_group
                .iter()
                .map(|iv| (iv.item_id.clone(), iv.value))
                .collect();
            (contract.min_value, values)
        };

        // We need idx again -- find it after the immutable borrow is done.
        let idx = {
            let Some(trade) = self.trade_state.get(region_name) else {
                return false;
            };
            trade
                .orders
                .iter()
                .position(|o| o.inst_id == inst_id as u32)
        };
        let Some(idx) = idx else {
            return false;
        };

        // Consume items from the bag.
        let Some(region) = self.region_mut(region_name) else {
            return false;
        };
        let Some(bag_node) = region.node_mut(1) else {
            return false;
        };
        let Some(bag_comp) = bag_node.component_mut(1) else {
            return false;
        };
        let crate::factory::FactoryComponent::Inventory(bag_inv) = bag_comp else {
            return false;
        };

        // Consume items from the bag. Collect what was actually consumed
        // so we can update the order after the region borrow is released.
        let mut consumed: Vec<(String, u32, u64)> = vec![]; // (item_id, count, value_added)
        for (item_id, &count) in items {
            if count <= 0 {
                continue;
            }
            let value_per = contract_item_values
                .iter()
                .find(|(id, _)| id == item_id)
                .map_or(0, |(_, v)| *v);

            if value_per == 0 {
                continue;
            }

            if try_consume_from_inv(bag_inv, item_id, count as u32) {
                consumed.push((item_id.clone(), count as u32, value_per * count as u64));
            }
        }

        // Drop the region borrow before touching trade_state.
        // bag_inv borrow ends here

        if consumed.is_empty() {
            return false;
        }

        // Now update the order with the consumed items.
        let Some(trade) = self.trade_state.get_mut(region_name) else {
            return false;
        };
        let Some(order) = trade.orders.get_mut(idx) else {
            return false;
        };

        let mut total_value = 0u64;
        for (item_id, count, value_added) in &consumed {
            total_value += value_added;
            order.accumulated_value += value_added;
            order.items_committed.push(ItemSlot {
                item_id: item_id.clone(),
                count: *count,
                inst_id: 0,
            });
        }

        if total_value == 0 {
            return false;
        }

        // Check if the order is now filled.
        let filled = {
            let Some(trade) = self.trade_state.get(region_name) else {
                return false;
            };
            trade
                .orders
                .get(idx)
                .is_some_and(|o| o.accumulated_value >= contract_min_value)
        };

        if filled {
            // Grant rewards from the contract's reward_items to the bag.
            // reward_items is a Vec<String> of item IDs, each granting 1 count.
            let reward_ids: Vec<String> = {
                let Some(trade) = self.trade_state.get(region_name) else {
                    return false;
                };
                let Some(contract_id) = &trade.active_contract else {
                    return false;
                };
                let Some(contract) = contracts.get_contract(contract_id) else {
                    return false;
                };
                contract.reward_items.clone()
            };

            let Some(region) = self.region_mut(region_name) else {
                return false;
            };
            if let Some(bag_node) = region.node_mut(1)
                && let Some(slot) = bag_node.component_mut(1)
                && let crate::factory::FactoryComponent::Inventory(inv_state) = slot
            {
                for reward_id in &reward_ids {
                    // Stack with existing same-item slots.
                    let mut stacked = false;
                    for existing in inv_state.items.values_mut() {
                        if existing.item_id == *reward_id {
                            existing.count += 1;
                            stacked = true;
                            break;
                        }
                    }
                    if !stacked {
                        inv_state.items.insert(
                            0,
                            ItemSlot {
                                item_id: reward_id.clone(),
                                count: 1,
                                inst_id: 0,
                            },
                        );
                    }
                }
            }

            let Some(trade) = self.trade_state.get_mut(region_name) else {
                return false;
            };
            trade.orders.remove(idx);
        }

        true
    }

    /// Cancel (delete) a trade order by inst_id.
    pub fn trade_delete_order(&mut self, region_name: &str, _node_id: u32, inst_id: u64) -> bool {
        let Some(trade) = self.trade_state.get_mut(region_name) else {
            return false;
        };
        let before = trade.orders.len();
        trade.orders.retain(|o| o.inst_id != inst_id as u32);
        trade.orders.len() < before
    }

    /// Lazy order generation -- called before cash_order so orders are
    /// up to date. Generates new orders based on elapsed time since
    /// `last_gen_tick`, using weighted-random selection from the
    /// contract's `orderIdGroup` + `orderWeight`.
    pub fn trade_generate_orders(
        &mut self,
        assets: &FTableAssets,
        contracts: &ContractAssets,
        region_name: &str,
    ) {
        let now = current_tick();
        let gen_speed = 360u64; // tradeOrderGenSpeed from FactoryConst
        let max_orders = 10usize; // from traderData.maxOrderCount

        let Some(trade) = self.trade_state.get_mut(region_name) else {
            return;
        };

        let Some(contract_id) = &trade.active_contract else {
            return;
        };
        let Some(contract) = contracts.get_contract(contract_id) else {
            return;
        };

        let elapsed = now.saturating_sub(trade.last_gen_tick);
        let new_slots = (elapsed / gen_speed) as usize;
        if new_slots == 0 {
            return;
        }

        let available = max_orders.saturating_sub(trade.orders.len());
        let to_generate = new_slots.min(available);

        // Generate a unique inst_id for each new order.
        let mut next_inst = trade.orders.iter().map(|o| o.inst_id).max().unwrap_or(0) + 1;

        for _ in 0..to_generate {
            // Weighted-random selection from orderIdGroup using orderWeight.
            let order_id = weighted_random(&contract.order_id_group, &contract.order_weight);
            if order_id.is_empty() {
                continue;
            }

            // Verify the order exists in FactoryTable.orderData.
            if assets.get_order(&order_id).is_none() {
                continue;
            }

            trade.orders.push(TradeOrder {
                order_id,
                accumulated_value: 0,
                items_committed: vec![],
                inst_id: next_inst,
            });
            next_inst += 1;
        }

        trade.last_gen_tick = now;
    }
}

/// Weighted random selection. Returns an empty string if the weights
/// are empty or all zero.
fn weighted_random(ids: &[String], weights: &[u32]) -> String {
    if ids.is_empty() || weights.is_empty() {
        return String::new();
    }

    let total: u32 = weights.iter().sum();
    if total == 0 {
        return ids.first().cloned().unwrap_or_default();
    }

    // Simple deterministic pick based on current_tick -- not truly random,
    // but good enough until we wire in a proper RNG. The live server uses
    // a seeded RNG; we can swap that in later.
    let seed = (current_tick() % total as u64) as u32;
    let mut acc = 0u32;
    for (i, &w) in weights.iter().enumerate() {
        acc += w;
        if seed < acc {
            return ids.get(i).cloned().unwrap_or_default();
        }
    }

    ids.last().cloned().unwrap_or_default()
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
