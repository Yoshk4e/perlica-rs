//! Manufacture queue business logic.
//!
//! Manufacture is a long-running craft (1-18 hours per recipe). The
//! player starts a recipe, it ticks in the background, and the player
//! settles (collects) the outcomes when done. Up to
//! `max_setCount` sets can be queued at once, and the outcome buffer
//! caps at `manufactOutcomeBufferStackMaxCount = 99`.

use config::factory_manufact_const::FManufactConstAssets;
use config::factory_table::FTableAssets;

use crate::factory::character_work::SKILL_TYPE_CRAFT_COST;
use crate::factory::{
    ActiveRecipe, ItemSlot, MachineKey, ManufactureMachine, Tick, current_tick, take_bill_from_inv,
};

/// Progress per tick for a manufacture recipe.
///
/// A recipe's `totalProgress` is its duration in ticks at speed 1. Named so a
/// building-level or skill speed-up has one place to land, and so the wire's
/// `current_speed` reports something real rather than a literal.
const MANUFACTURE_SPEED: u64 = 1;

impl crate::factory::FactoryManager {
    /// Start a manufacture recipe. Consumes ingredients from the bag,
    /// sets the active recipe + start_tick. If a recipe was already
    /// running, the old one is cancelled first (outcomes retained).
    pub fn manufacture_start(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        node_id: u32,
        formula_id: &str,
        count: u32,
    ) -> Result<ManufactureStartResult, ManufactureError> {
        let recipe = assets
            .get_manufact_craft(formula_id)
            .ok_or(ManufactureError::RecipeNotFound)?
            .clone();

        let max_sets = assets_factory_manufact_max_sets(assets);
        let set_count = count.min(max_sets);

        // The node's template id is the wire `machine_id`. Resolve it before
        // consuming anything, so a start against an unplaced node fails
        // without eating the player's ingredients.
        let machine_id = self
            .machine_id_of(region_name, node_id)
            .ok_or(ManufactureError::MachineNotFound)?
            .to_string();

        // The rate this round is charged at, resolved before the bag is
        // touched. Banked on the machine so the client's "current round" scale
        // stays the one the player actually paid, even if the roster changes
        // mid-round.
        let round_cost_delta_scale =
            self.skill_cost_scale(assets, region_name, &machine_id, SKILL_TYPE_CRAFT_COST);

        // Price the whole bill before touching the bag: the old per-ingredient
        // loop bailed out mid-way on a short bag, having already eaten the
        // ingredients it managed to find.
        let bill: Vec<(&str, u32)> = recipe
            .ingredients
            .iter()
            .map(|ing| (ing.id.as_str(), ing.count.saturating_mul(set_count)))
            .collect();

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
        if !take_bill_from_inv(bag_inv, &bill) {
            return Err(ManufactureError::InsufficientIngredients);
        }
        // bag_inv borrow ends here

        let machine = self
            .manufacture_state
            .entry(MachineKey::new(region_name, node_id))
            .or_insert(ManufactureMachine {
                region_name: region_name.to_string(),
                node_id,
                machine_id,
                building_level: 1,
                active_recipe: None,
                outcome_buffer: vec![],
                set_count: 0,
                round_cost_delta_scale: 1.0,
            });

        let old = machine.active_recipe.take();
        let old_sets = machine.set_count;
        let now = current_tick();

        // What the outgoing recipe had already finished, for the client's
        // "here is what you lost" notice.
        let (old_formula, old_got, old_least_multi) =
            describe_discarded(assets, old.as_ref(), old_sets, now);

        machine.active_recipe = Some(ActiveRecipe::start_at(now, formula_id, MANUFACTURE_SPEED));
        machine.set_count = set_count;
        machine.round_cost_delta_scale = round_cost_delta_scale;

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
        node_id: u32,
    ) -> Result<ManufactureCancelResult, ManufactureError> {
        let machine = self
            .manufacture_state
            .get_mut(&MachineKey::new(region_name, node_id))
            .ok_or(ManufactureError::MachineNotFound)?;

        let old = machine.active_recipe.take();
        let old_sets = machine.set_count;
        machine.set_count = 0;

        let (old_formula, old_got, old_least_multi) =
            describe_discarded(assets, old.as_ref(), old_sets, current_tick());

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
        node_id: u32,
    ) -> Result<ManufactureSettleResult, ManufactureError> {
        let machine = self
            .manufacture_state
            .get_mut(&MachineKey::new(region_name, node_id))
            .ok_or(ManufactureError::MachineNotFound)?;

        let Some(recipe_ref) = machine.active_recipe.as_ref() else {
            return Ok(ManufactureSettleResult {
                settle_count: 0,
                auto_supple_count: 0,
            });
        };

        let recipe_id = recipe_ref.recipe_id.clone();

        let recipe = assets
            .get_manufact_craft(&recipe_id)
            .ok_or(ManufactureError::RecipeNotFound)?;

        let now = current_tick();
        let queued = u64::from(machine.set_count);
        let completed_sets = machine.active_recipe.as_ref().map_or(0, |r| {
            r.timing.completed_rounds_at(now, recipe.total_progress)
        });

        let available_sets = u32::try_from(completed_sets.min(queued)).unwrap_or(u32::MAX);
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

        // Charge the clock only for what actually landed in the buffer. Sets
        // that ran but had nowhere to go stay banked in the timing, so a full
        // buffer defers the player's output instead of destroying it.
        if let Some(active_recipe) = machine.active_recipe.as_mut() {
            active_recipe.timing.take_completed_rounds_capped_at(
                now,
                recipe.total_progress,
                u64::from(settled),
            );
        }

        machine.set_count = machine.set_count.saturating_sub(settled);
        if machine.set_count == 0 {
            machine.active_recipe = None;
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
    InsufficientIngredients,
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

/// Describe a recipe being thrown away: `(formula_id, sets_finished,
/// round_count)`, matching the wire's `old_formula` / `old_got` /
/// `old_least_multi` trio.
///
/// Read-only — the recipe is gone, so there is nothing to bank. `queued_sets`
/// caps the count because a recipe can only ever have finished as many sets as
/// the player paid ingredients for, however long it sat there.
fn describe_discarded(
    assets: &FTableAssets,
    old: Option<&ActiveRecipe>,
    queued_sets: u32,
    now: Tick,
) -> (String, i32, i32) {
    let Some(old) = old else {
        return (String::new(), 0, 0);
    };

    let entry = assets.get_manufact_craft(&old.recipe_id);
    let completed = entry.map_or(0, |e| {
        let rounds = old
            .timing
            .completed_rounds_at(now, e.total_progress)
            .min(u64::from(queued_sets));
        i32::try_from(rounds).unwrap_or(i32::MAX)
    });
    let round_count = entry.map_or(1, |e| e.round_count as i32);

    (old.recipe_id.clone(), completed, round_count)
}
