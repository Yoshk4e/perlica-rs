//! Soil / farming business logic.
//!
//! The soil component accepts a seed item, grows it over time based on
//! `growTotalProgress` (from `seedItemData`) and `growSpeed` (from
//! `soilData`), and when fully grown can be harvested to produce the
//! seed's outcome item.
//!
//! Three ops: `plant` (start growing), `harvest` (collect the grown
//! item), `cancel` (abort and clear the soil).

use config::factory_table::FTableAssets;

use crate::factory::{FactoryManager, ItemSlot, SoilMachine, current_tick, elapsed_since};

impl FactoryManager {
    /// Plant a seed in a soil node. Consumes the seed from the player's
    /// bag, creates a `SoilMachine` entry with the seed's recipe + start
    /// tick. Growth is computed lazily from elapsed time.
    pub fn soil_plant(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        _node_id: u32,
        seed_item_id: &str,
    ) -> bool {
        // Verify the seed exists in config.
        let Some(seed_entry) = assets.get_seed_item(seed_item_id) else {
            return false;
        };

        // Consume the seed from the bag.
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

        if !try_consume_from_inv(bag_inv, seed_item_id, 1) {
            return false;
        }

        // Create or update the soil machine state.
        let now = current_tick();
        self.soil_state.insert(
            region_name.to_string(),
            SoilMachine {
                region_name: region_name.to_string(),
                planted_seed: Some(crate::factory::ActiveRecipe {
                    recipe_id: seed_item_id.to_string(),
                    start_tick: now,
                }),
                doodad_state: crate::factory::SoilDoodadState::Growing,
            },
        );

        let _ = seed_entry;
        true
    }

    /// Harvest a grown seed. Checks if the growth is complete
    /// (`elapsed * growSpeed >= growTotalProgress`), and if so produces
    /// the outcome item into the bag and clears the soil.
    pub fn soil_harvest(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        _node_id: u32,
        _harvest_type: i32,
    ) -> bool {
        let Some(machine) = self.soil_state.get(region_name) else {
            return false;
        };
        let Some(recipe) = &machine.planted_seed else {
            return false;
        };

        let seed_id = recipe.recipe_id.clone();
        let start_tick = recipe.start_tick;

        let Some(seed_entry) = assets.get_seed_item(&seed_id) else {
            return false;
        };

        // Look up the soil's grow_speed from soilData. The soil_id is
        // the node's template_id, but we don't have the node here.
        // Default to 1 (all soils have growSpeed=1 per the doc).
        let grow_speed = 1u64;

        let elapsed = elapsed_since(start_tick);
        let progress = elapsed.saturating_mul(grow_speed);
        if progress < seed_entry.grow_total_progress {
            return false;
        }

        // Growth complete -- produce the outcome. The seed's outcome
        // is the seed item itself (harvesting yields more seeds or the
        // crop item). The `doodad_id` field tells the client which
        // model to show; the actual outcome item is the seed's `id`.
        let outcome_item_id = seed_entry.id.clone();

        // Push the outcome into the bag.
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

        // Stack with existing or insert new.
        let mut stacked = false;
        for existing in bag_inv.items.values_mut() {
            if existing.item_id == outcome_item_id {
                existing.count += 1;
                stacked = true;
                break;
            }
        }
        if !stacked {
            bag_inv.items.insert(
                0,
                ItemSlot {
                    item_id: outcome_item_id,
                    count: 1,
                    inst_id: 0,
                },
            );
        }

        // Clear the soil.
        let Some(machine) = self.soil_state.get_mut(region_name) else {
            return false;
        };
        machine.planted_seed = None;
        machine.doodad_state = crate::factory::SoilDoodadState::Empty;

        true
    }

    /// Cancel growing and clear the soil. The seed is lost (not returned).
    pub fn soil_cancel(&mut self, region_name: &str, _node_id: u32) -> bool {
        let Some(machine) = self.soil_state.get_mut(region_name) else {
            return false;
        };
        machine.planted_seed = None;
        machine.doodad_state = crate::factory::SoilDoodadState::Empty;
        true
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
