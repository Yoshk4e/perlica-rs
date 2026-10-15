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

use crate::factory::character_work::SKILL_TYPE_TRADE_COST;
use crate::factory::{
    FactoryManager, ItemSlot, MachineKey, TradeOrder, current_tick, push_item_to_bag, take_from_inv,
};

/// Progress per tick for the order clock.
///
/// The generation interval (`tradeOrderGenSpeed`) is a duration in ticks, so
/// the clock accrues 1:1 and one *round* of `TRADE_ORDER_GEN_INTERVAL`
/// progress is one order slot.
const TRADE_GEN_SPEED: u64 = 1;

/// `tradeOrderGenSpeed` from `FactoryConst`: ticks per generated order.
const TRADE_ORDER_GEN_INTERVAL: u64 = 360;

/// `traderData.maxOrderCount`: how many orders may be pending at once.
const TRADE_MAX_ORDERS: usize = 10;

impl FactoryManager {
    /// Set the active contract for a trader. Replaces any previously
    /// active contract and clears pending orders.
    pub fn trade_set_contract(
        &mut self,
        contracts: &ContractAssets,
        region_name: &str,
        node_id: u32,
        contract_id: &str,
    ) -> bool {
        if contracts.get_contract(contract_id).is_none() {
            return false;
        }

        // The node's template id is the wire `machine_id`; resolve it before
        // the mutable borrow, and reject ops against an unplaced node.
        let Some(machine_id) = self.machine_id_of(region_name, node_id).map(str::to_string) else {
            return false;
        };

        let now = current_tick();
        let trade = self
            .trade_state
            .entry(MachineKey::new(region_name, node_id))
            .or_insert(crate::factory::TradeMachine {
                region_name: region_name.to_string(),
                node_id,
                machine_id,
                building_level: 1,
                active_contract: None,
                orders: vec![],
                order_timing: crate::factory::MachineTiming::start_at(now, TRADE_GEN_SPEED),
            });

        trade.active_contract = Some(contract_id.to_string());
        trade.orders.clear();
        // A new contract starts its order clock fresh: the orders it would have
        // generated are the old contract's, and they were just cleared.
        trade.order_timing.restart_at(now);
        trade.order_timing.resume_at(now);
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
        node_id: u32,
        inst_id: u64,
        items: &std::collections::HashMap<String, i64>,
    ) -> bool {
        let key = MachineKey::new(region_name, node_id);

        // First, generate any pending orders (lazy tick).
        self.trade_generate_orders(assets, contracts, region_name, node_id);

        // Look up contract + order index without holding a region borrow.
        let (contract_min_value, contract_item_values) = {
            let Some(trade) = self.trade_state.get(&key) else {
                return false;
            };
            let Some(contract_id) = &trade.active_contract else {
                return false;
            };
            let Some(contract) = contracts.get_contract(contract_id) else {
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

        let Some(idx) = self
            .trade_state
            .get(&key)
            .and_then(|trade| trade.orders.iter().position(|o| o.inst_id == inst_id))
        else {
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

            // Each entry commits on its own: an order can be part-filled from
            // whatever the bag actually holds. The take is still all-or-nothing
            // per item, so a stack is never eaten without crediting its value.
            if take_from_inv(bag_inv, item_id, count as u32) {
                consumed.push((item_id.clone(), count as u32, value_per * count as u64));
            }
        }

        // Drop the region borrow before touching trade_state.
        // bag_inv borrow ends here

        if consumed.is_empty() {
            return false;
        }

        // Now update the order with the consumed items.
        let Some(trade) = self.trade_state.get_mut(&key) else {
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
            let Some(trade) = self.trade_state.get(&key) else {
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
                let Some(trade) = self.trade_state.get(&key) else {
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
                    push_item_to_bag(inv_state, reward_id, 1, 0);
                }
            }

            let Some(trade) = self.trade_state.get_mut(&key) else {
                return false;
            };
            trade.orders.remove(idx);
        }

        true
    }

    /// Cancel (delete) a trade order by inst_id.
    pub fn trade_delete_order(&mut self, region_name: &str, node_id: u32, inst_id: u64) -> bool {
        let Some(trade) = self
            .trade_state
            .get_mut(&MachineKey::new(region_name, node_id))
        else {
            return false;
        };
        let before = trade.orders.len();
        trade.orders.retain(|o| o.inst_id != inst_id);
        trade.orders.len() < before
    }

    /// Lazy order generation -- called before cash_order so orders are
    /// up to date. Generates new orders based on time accrued in
    /// `order_timing`, using weighted-random selection from the
    /// contract's `orderIdGroup` + `orderWeight`.
    pub fn trade_generate_orders(
        &mut self,
        assets: &FTableAssets,
        contracts: &ContractAssets,
        region_name: &str,
        node_id: u32,
    ) {
        let now = current_tick();

        // Price the orders about to be generated at the loadout that is punched
        // in *now*, before the mutable borrow. Each order banks its own scale,
        // so a roster change later does not silently reprice work in flight.
        let cost_scale = self.machine_id_of(region_name, node_id).map_or(1.0, |id| {
            self.skill_cost_scale(assets, region_name, id, SKILL_TYPE_TRADE_COST)
        });

        let Some(trade) = self
            .trade_state
            .get_mut(&MachineKey::new(region_name, node_id))
        else {
            return;
        };

        let Some(contract_id) = &trade.active_contract else {
            return;
        };
        let Some(contract) = contracts.get_contract(contract_id) else {
            return;
        };

        // Read the elapsed slots without consuming them: a slot is only paid
        // for once an order is actually pushed. The old code assigned
        // `last_gen_tick = now` unconditionally, which discarded both the
        // sub-interval remainder and every slot that overflowed `max_orders`.
        let available = TRADE_MAX_ORDERS.saturating_sub(trade.orders.len()) as u64;
        let pending = trade
            .order_timing
            .completed_rounds_at(now, TRADE_ORDER_GEN_INTERVAL)
            .min(available);
        if pending == 0 {
            return;
        }

        // Generate a unique inst_id for each new order.
        let mut next_inst = trade.orders.iter().map(|o| o.inst_id).max().unwrap_or(0) + 1;

        let mut generated = 0u64;
        for _ in 0..pending {
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
                contract_id: contract_id.clone(),
                cost_value_delta: cost_scale,
                inst_id: next_inst,
            });
            next_inst += 1;
            generated += 1;
        }

        // Charge only for the orders that materialised. A contract pointing at
        // ids missing from `orderData` is a config fault; burning the player's
        // accrued time over it would hide the fault and cost them orders.
        trade.order_timing.take_completed_rounds_capped_at(
            now,
            TRADE_ORDER_GEN_INTERVAL,
            generated,
        );
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
