//! Manual work business logic.
//!
//! Manual crafting is a player-driven queue (max 5 items, each craft
//! takes `totalProgress = 120` ticks at speed 1 = 2 minutes). The
//! queue processes automatically when not paused. Pause/resume/cancel
//! control the queue state.
//!
//! Four ops: `append` (add to queue), `cancel` (clear + return
//! ingredients), `pause`, `resume`.

use config::factory_table::FTableAssets;

use crate::factory::{FactoryManager, ItemSlot, ManualWorkUnit, current_tick, elapsed_since};

impl FactoryManager {
    /// Append a manual craft to the queue. Consumes ingredients from
    /// the bag. Capped at `manualCraftQueueLength = 5`.
    pub fn manual_work_append(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        formula_id: &str,
        count: i32,
    ) -> bool {
        let Some(recipe) = assets.get_manual_craft(formula_id) else {
            return false;
        };

        // Check queue cap.
        let max_queue = 5usize;
        if self.manual_work_state.queue.len() >= max_queue {
            return false;
        }

        let count = count.max(1) as u32;

        // Consume ingredients from the bag.
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

        for ingredient in &recipe.ingredients {
            let needed = ingredient.count.saturating_mul(count);
            if !try_consume_from_inv(bag_inv, &ingredient.id, needed) {
                return false;
            }
        }

        // Add to queue. Each unit starts ticking from now.
        let now = current_tick();
        for _ in 0..count {
            self.manual_work_state.queue.push(ManualWorkUnit {
                recipe_id: formula_id.to_string(),
                start_tick: now,
                progress: 0,
            });
        }

        true
    }

    /// Cancel the manual work queue. Returns the ingredients to refund
    /// and the items that would have broken (lost).
    pub fn manual_work_cancel(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
    ) -> (
        std::collections::HashMap<String, i32>,
        std::collections::HashMap<String, i32>,
    ) {
        let mut back_items: std::collections::HashMap<String, i32> =
            std::collections::HashMap::new();
        let break_items: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

        // For each queued unit, refund its ingredients.
        for unit in &self.manual_work_state.queue {
            if let Some(recipe) = assets.get_manual_craft(&unit.recipe_id) {
                for ingredient in &recipe.ingredients {
                    *back_items.entry(ingredient.id.clone()).or_insert(0) +=
                        ingredient.count as i32;
                }
            }
        }

        // Push refunded items back into the bag.
        if let Some(region) = self.region_mut(region_name)
            && let Some(bag_node) = region.node_mut(1)
            && let Some(bag_comp) = bag_node.component_mut(1)
            && let crate::factory::FactoryComponent::Inventory(inv_state) = bag_comp
        {
            for (item_id, count) in &back_items {
                let mut stacked = false;
                for slot in inv_state.items.values_mut() {
                    if slot.item_id == *item_id {
                        slot.count += *count as u32;
                        stacked = true;
                        break;
                    }
                }
                if !stacked {
                    inv_state.items.insert(
                        0,
                        ItemSlot {
                            item_id: item_id.clone(),
                            count: *count as u32,
                            inst_id: 0,
                        },
                    );
                }
            }
        }

        self.manual_work_state.queue.clear();
        self.manual_work_state.is_paused = false;

        (back_items, break_items)
    }

    /// Pause the manual work queue.
    pub fn manual_work_pause(&mut self) {
        self.manual_work_state.is_paused = true;
    }

    /// Resume the manual work queue.
    pub fn manual_work_resume(&mut self) {
        if self.manual_work_state.is_paused {
            // Reset the start_tick of the head unit so it resumes from now.
            if let Some(head) = self.manual_work_state.queue.first_mut() {
                head.start_tick = current_tick();
            }
            self.manual_work_state.is_paused = false;
        }
    }

    /// Tick the manual work queue. Completes units whose
    /// `elapsed >= totalProgress` and produces outcomes into the bag.
    /// Called by the server tick loop.
    pub fn tick_manual_work(&mut self, assets: &FTableAssets, region_name: &str) -> usize {
        if self.manual_work_state.is_paused || self.manual_work_state.queue.is_empty() {
            return 0;
        }

        let mut completed = 0;
        let now = current_tick();

        // Check the head unit.
        let (recipe_id, total_progress, is_done) = {
            let head = &self.manual_work_state.queue[0];
            let Some(recipe) = assets.get_manual_craft(&head.recipe_id) else {
                // Recipe vanished from config -- drop the unit.
                return 0;
            };
            let elapsed = elapsed_since(head.start_tick);
            (
                head.recipe_id.clone(),
                recipe.total_progress,
                elapsed >= total_progress_for(recipe),
            )
        };

        let _ = total_progress;

        if is_done {
            // Produce outcomes into the bag.
            let Some(recipe) = assets.get_manual_craft(&recipe_id) else {
                return 0;
            };

            let Some(region) = self.region_mut(region_name) else {
                return 0;
            };
            if let Some(bag_node) = region.node_mut(1)
                && let Some(bag_comp) = bag_node.component_mut(1)
                && let crate::factory::FactoryComponent::Inventory(inv_state) = bag_comp
            {
                for outcome in &recipe.outcomes {
                    let mut stacked = false;
                    for slot in inv_state.items.values_mut() {
                        if slot.item_id == outcome.id {
                            slot.count += outcome.count;
                            stacked = true;
                            break;
                        }
                    }
                    if !stacked {
                        inv_state.items.insert(
                            0,
                            ItemSlot {
                                item_id: outcome.id.clone(),
                                count: outcome.count,
                                inst_id: 0,
                            },
                        );
                    }
                }
            }

            // Remove the completed unit.
            self.manual_work_state.queue.remove(0);

            // Start the next unit if any.
            if let Some(next) = self.manual_work_state.queue.first_mut() {
                next.start_tick = now;
            }

            completed += 1;
        }

        completed
    }
}

fn total_progress_for(recipe: &config::tables::factory_table::ManualCraftEntry) -> u64 {
    recipe.total_progress
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
