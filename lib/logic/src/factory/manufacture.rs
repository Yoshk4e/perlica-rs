//! Manufacture queue business logic.
//!
//! Manufacture is a long-running craft (1-18 hours per recipe). The
//! player starts a recipe, it ticks in the background, and the player
//! settles (collects) the outcomes when done. Up to
//! `max_setCount` sets can be queued at once, and the outcome buffer
//! caps at `manufactOutcomeBufferStackMaxCount = 99`.

use config::factory_manufact_const::FManufactConstAssets;
use config::factory_table::FTableAssets;

use crate::factory::{
    ActiveRecipe, InventoryState, ItemSlot, ManufactureMachine, current_tick, elapsed_since,
};

impl crate::factory::FactoryManager {
    /// Start a manufacture recipe. Consumes ingredients from the bag,
    /// sets the active recipe + start_tick. If a recipe was already
    /// running, the old one is cancelled first (outcomes retained).
    pub fn manufacture_start(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        _node_id: u32,
        formula_id: &str,
        count: u32,
    ) -> Result<ManufactureStartResult, ManufactureError> {
        let recipe = assets
            .get_manufact_craft(formula_id)
            .ok_or(ManufactureError::RecipeNotFound)?
            .clone();

        let max_sets = assets_factory_manufact_max_sets(assets);
        let set_count = count.min(max_sets);

        // Consume ingredients from the bag.
        let Some(region) = self.region_mut(region_name) else {
            return Err(ManufactureError::MachineNotFound);
        };
        let Some(bag_node) = region.node_mut(1) else {
            return Err(ManufactureError::MachineNotFound);
        };
        let Some(bag_comp) = bag_node.component_mut(1) else {
            return Err(ManufactureError::MachineNotFound);
        };
        let crate::factory::FactoryComponent::Inventory(bag_inv) = bag_comp else {
            return Err(ManufactureError::MachineNotFound);
        };
        for ingredient in &recipe.ingredients {
            let needed = ingredient.count.saturating_mul(set_count);
            if !try_consume_from_inv(bag_inv, &ingredient.id, needed) {
                return Err(ManufactureError::MachineNotFound);
            }
        }
        // bag_inv borrow ends here

        let machine = self
            .manufacture_state
            .entry(region_name.to_string())
            .or_insert(ManufactureMachine {
                region_name: region_name.to_string(),
                building_level: 1,
                active_recipe: None,
                outcome_buffer: vec![],
                set_count: 0,
            });

        let old = machine.active_recipe.take();

        // Compute how many sets were completed from the old recipe.
        let (old_formula, old_got, old_least_multi) = if let Some(ref old_recipe) = old {
            let old_recipe_entry = assets.get_manufact_craft(&old_recipe.recipe_id);
            let completed = if let Some(entry) = old_recipe_entry {
                let elapsed = elapsed_since(old_recipe.start_tick);
                u32::try_from(elapsed / entry.total_progress.max(1)).unwrap_or(0)
            } else {
                0
            };
            let round_count = old_recipe_entry.map_or(1, |e| e.round_count as i32);
            (old_recipe.recipe_id.clone(), completed as i32, round_count)
        } else {
            (String::new(), 0, 0)
        };

        machine.active_recipe = Some(ActiveRecipe {
            recipe_id: formula_id.to_string(),
            start_tick: current_tick(),
        });
        machine.set_count = set_count;

        Ok(ManufactureStartResult {
            old_formula,
            old_got,
            old_least_multi,
        })
    }

    /// Cancel the active manufacture recipe. Returns the old recipe info
    /// so the client can update its UI.
    pub fn manufacture_cancel(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        _node_id: u32,
    ) -> Result<ManufactureCancelResult, ManufactureError> {
        let machine = self
            .manufacture_state
            .get_mut(region_name)
            .ok_or(ManufactureError::MachineNotFound)?;

        let old = machine.active_recipe.take();
        machine.set_count = 0;

        let (old_formula, old_got, old_least_multi) = if let Some(ref old_recipe) = old {
            let old_recipe_entry = assets.get_manufact_craft(&old_recipe.recipe_id);
            let completed = if let Some(entry) = old_recipe_entry {
                let elapsed = elapsed_since(old_recipe.start_tick);
                u32::try_from(elapsed / entry.total_progress.max(1)).unwrap_or(0)
            } else {
                0
            };
            let round_count = old_recipe_entry.map_or(1, |e| e.round_count as i32);
            (old_recipe.recipe_id.clone(), completed as i32, round_count)
        } else {
            (String::new(), 0, 0)
        };

        Ok(ManufactureCancelResult {
            old_formula,
            old_got,
            old_least_multi,
        })
    }

    /// Settle (collect) completed manufacture outcomes. Checks if the
    /// recipe has finished based on elapsed time vs `total_progress`,
    /// and if so moves the outcome items into the outcome buffer (capped
    /// at `manufactOutcomeBufferStackMaxCount`).
    pub fn manufacture_settle(
        &mut self,
        assets: &FTableAssets,
        manufact_const: &FManufactConstAssets,
        region_name: &str,
        _node_id: u32,
    ) -> Result<ManufactureSettleResult, ManufactureError> {
        let machine = self
            .manufacture_state
            .get_mut(region_name)
            .ok_or(ManufactureError::MachineNotFound)?;

        let Some(recipe_ref) = machine.active_recipe.as_ref() else {
            return Ok(ManufactureSettleResult {
                settle_count: 0,
                auto_supple_count: 0,
            });
        };

        let recipe_id = recipe_ref.recipe_id.clone();
        let start_tick = recipe_ref.start_tick;

        let recipe = assets
            .get_manufact_craft(&recipe_id)
            .ok_or(ManufactureError::RecipeNotFound)?;

        let elapsed = elapsed_since(start_tick);
        let completed_sets = u32::try_from(elapsed / recipe.total_progress.max(1)).unwrap_or(0);

        let available_sets = completed_sets.min(machine.set_count);
        if available_sets == 0 {
            return Ok(ManufactureSettleResult {
                settle_count: 0,
                auto_supple_count: 0,
            });
        }

        let max_stack = manufact_const.data.manufact_outcome_buffer_stack_max_count;

        let mut settled = 0u32;
        for _ in 0..available_sets {
            let outcome = &recipe.outcome;
            let outcome_item = ItemSlot {
                item_id: outcome.id.clone(),
                count: outcome.count,
                inst_id: 0,
            };

            let existing = machine
                .outcome_buffer
                .iter_mut()
                .find(|s| s.item_id == outcome_item.item_id);

            if let Some(slot) = existing {
                if slot.count + outcome_item.count <= max_stack {
                    slot.count += outcome_item.count;
                    settled += 1;
                } else {
                    break;
                }
            } else if machine.outcome_buffer.len() < 99 {
                machine.outcome_buffer.push(outcome_item);
                settled += 1;
            } else {
                break;
            }
        }

        machine.set_count = machine.set_count.saturating_sub(settled);
        if machine.set_count == 0 {
            machine.active_recipe = None;
        } else if settled > 0 {
            // Reset start_tick for the remaining sets.
            if let Some(recipe) = machine.active_recipe.as_mut() {
                recipe.start_tick = current_tick();
            }
        }

        Ok(ManufactureSettleResult {
            settle_count: settled as i32,
            auto_supple_count: 0,
        })
    }
}

#[derive(Debug)]
pub enum ManufactureError {
    RecipeNotFound,
    MachineNotFound,
}

#[derive(Debug)]
pub struct ManufactureStartResult {
    pub old_formula: String,
    pub old_got: i32,
    pub old_least_multi: i32,
}

#[derive(Debug)]
pub struct ManufactureCancelResult {
    pub old_formula: String,
    pub old_got: i32,
    pub old_least_multi: i32,
}

#[derive(Debug)]
pub struct ManufactureSettleResult {
    pub settle_count: i32,
    pub auto_supple_count: i32,
}

fn assets_factory_manufact_max_sets(_assets: &FTableAssets) -> u32 {
    // Default to 4 if the const isn't available.
    4
}

fn try_consume_from_inv(inv: &mut InventoryState, item_id: &str, needed: u32) -> bool {
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
