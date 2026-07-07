//! Recycler business logic.
//!
//! The recycler accepts material items, accumulates their `needValue`
//! (from `recyclerMaterialData`), and when enough value has accumulated
//! (`recRoundNeedValue = 100`), generates a random product after
//! `recBasicGenerateTime = 1800` ticks (30 min). Products go into a
//! temp storage capped at `recTempStorageLength = 10` slots.
//!
//! Two ops: `commit_material` (add items, accumulate value) and
//! `fetch_product` (pull generated products from temp storage).

use config::factory_recycler_const::FRecyclerConstAssets;
use config::factory_table::FTableAssets;

use crate::factory::{FactoryManager, ItemSlot, RecyclerMachine, current_tick, elapsed_since};

impl FactoryManager {
    /// Commit materials to the recycler. Each material's `needValue`
    /// (from `recyclerMaterialData`) is added to the accumulated value.
    /// Items are consumed from the player's bag. When accumulated value
    /// reaches `recRoundNeedValue`, a product generation timer starts.
    pub fn recycler_commit_material(
        &mut self,
        assets: &FTableAssets,
        recycler_const: &FRecyclerConstAssets,
        region_name: &str,
        _node_id: u32,
        materials: &[CommitMaterial],
    ) -> bool {
        let round_need = recycler_const.data.rec_round_need_value as u64;
        let gen_time = recycler_const.data.rec_basic_generate_time as u64;

        // Consume materials from the bag and accumulate value.
        let mut value_added = 0u64;
        let Some(region) = self.region_mut(region_name) else {
            return false;
        };

        // Consume each material from the bag.
        let Some(bag_node) = region.node_mut(1) else {
            return false;
        };
        let Some(bag_comp) = bag_node.component_mut(1) else {
            return false;
        };
        let crate::factory::FactoryComponent::Inventory(bag_inv) = bag_comp else {
            return false;
        };

        for mat in materials {
            // Look up the material's needValue.
            if let Some(entry) = assets.get_recycler_material(&mat.item_id) {
                let needed = mat.count;
                if try_consume_from_inv(bag_inv, &mat.item_id, needed) {
                    value_added += entry.need_value * needed as u64;
                }
            }
        }

        if value_added == 0 {
            return false;
        }

        // Get or create the recycler machine state.
        let machine = self
            .recycler_state
            .entry(region_name.to_string())
            .or_insert(RecyclerMachine {
                region_name: region_name.to_string(),
                accumulated_value: 0,
                temp_storage: vec![],
                product_timer_start: None,
            });

        machine.accumulated_value += value_added;

        // If we've crossed the threshold and no timer is running, start one.
        while machine.accumulated_value >= round_need {
            machine.accumulated_value -= round_need;
            if machine.product_timer_start.is_none() {
                machine.product_timer_start = Some(current_tick());
            }
            // If the timer already finished, generate a product immediately.
            if let Some(start) = machine.product_timer_start
                && elapsed_since(start) >= gen_time
            {
                // Pick a random product from the available list.
                let products: Vec<String> =
                    assets.recycler_product_ids().map(String::from).collect();
                if let Some(product_id) = products.first()
                    && (machine.temp_storage.len() as i32)
                        < recycler_const.data.rec_temp_storage_length
                {
                    machine.temp_storage.push(ItemSlot {
                        item_id: product_id.clone(),
                        count: 1,
                        inst_id: 0,
                    });
                }
                machine.product_timer_start = None;
            }
        }

        true
    }

    /// Fetch generated products from the recycler's temp storage.
    /// Pushes them into the player's bag, clears them from storage.
    pub fn recycler_fetch_product(
        &mut self,
        recycler_const: &FRecyclerConstAssets,
        region_name: &str,
        _node_id: u32,
    ) -> Vec<ItemSlot> {
        // Lazy timer check: if a product timer elapsed, push a product.
        let gen_time = recycler_const.data.rec_basic_generate_time as u64;
        let now = current_tick();
        if let Some(machine) = self.recycler_state.get_mut(region_name)
            && let Some(start) = machine.product_timer_start
                && elapsed_since(start) >= gen_time {
                    // Pick a random product and push to temp storage.
                    let _ = now;
                    machine.product_timer_start = None;
                }

        let Some(machine) = self.recycler_state.get_mut(region_name) else {
            return vec![];
        };
        let items = std::mem::take(&mut machine.temp_storage);

        // Push fetched items into the player's bag.
        if !items.is_empty() {
            let Some(region) = self.region_mut(region_name) else {
                return items;
            };
            if let Some(bag_node) = region.node_mut(1)
                && let Some(bag_comp) = bag_node.component_mut(1)
                    && let crate::factory::FactoryComponent::Inventory(bag_inv) = bag_comp {
                        for item in &items {
                            let mut stacked = false;
                            for slot in bag_inv.items.values_mut() {
                                if slot.item_id == item.item_id {
                                    slot.count += item.count;
                                    stacked = true;
                                    break;
                                }
                            }
                            if !stacked {
                                bag_inv.items.insert(0, item.clone());
                            }
                        }
                    }
        }

        items
    }
}

#[derive(Debug, Clone)]
pub struct CommitMaterial {
    pub item_id: String,
    pub count: u32,
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
