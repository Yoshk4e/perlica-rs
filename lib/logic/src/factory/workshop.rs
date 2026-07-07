//! Workshop crafting business logic.
//!
//! The workshop produces buildings and items from recipes in
//! `workshopCraftData`. Each recipe has ingredients, outcomes, and a
//! `default_unlock` flag. Recipes that aren't `default_unlock` need to
//! be unlocked via STT (Clause 17) before they can be crafted.
//!
//! `multi` on the request means "craft N times" -- ingredients are
//! consumed N times and outcomes produced N times.

use config::factory_table::FTableAssets;

use crate::factory::{FactoryComponent, FactoryManager, ItemSlot};

impl FactoryManager {
    /// Craft an item via the workshop. Consumes ingredients from the
    /// player's bag (inventory node), produces outcomes into the bag.
    /// Returns the produced items, or an error if the recipe doesn't
    /// exist, isn't unlocked, or ingredients are insufficient.
    pub fn workshop_make(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        formula_id: &str,
        multi: u32,
    ) -> Result<Vec<ItemSlot>, WorkshopError> {
        let recipe = assets
            .get_workshop_craft(formula_id)
            .ok_or(WorkshopError::RecipeNotFound)?;

        // Check unlock -- if not default_unlock, the player needs it in
        // their unlocked formulas list. TODO: wire STT unlock check
        // once Clause 17 lands. For now, only default_unlock recipes
        // are craftable.
        if !recipe.default_unlock {
            return Err(WorkshopError::NotUnlocked);
        }

        let multi = multi.max(1);

        // Consume ingredients from the bag (inventory node_id=1).
        let region = self
            .region_mut(region_name)
            .ok_or(WorkshopError::RegionNotFound)?;

        let bag_node = region.node_mut(1).ok_or(WorkshopError::InventoryMissing)?;
        let bag_comp = bag_node
            .component_mut(1)
            .ok_or(WorkshopError::InventoryMissing)?;
        let FactoryComponent::Inventory(bag_inv) = bag_comp else {
            return Err(WorkshopError::InventoryMissing);
        };

        // Check + consume each ingredient.
        for ingredient in &recipe.ingredients {
            let needed = ingredient.count.saturating_mul(multi);
            if !try_consume_from_inventory(bag_inv, &ingredient.id, needed) {
                return Err(WorkshopError::InsufficientIngredients);
            }
        }

        // Produce outcomes into the bag.
        let mut produced = vec![];
        for outcome in &recipe.outcomes {
            let count = outcome.count.saturating_mul(multi);
            produced.push(ItemSlot {
                item_id: outcome.id.clone(),
                count,
                inst_id: 0,
            });
        }

        // Push outcomes into the bag.
        for item in &produced {
            if let Some(existing) = bag_inv.items.get_mut(&0)
                && existing.item_id == item.item_id {
                    existing.count += item.count;
                    continue;
                }
            bag_inv.items.insert(0, item.clone());
        }

        Ok(produced)
    }
}

#[derive(Debug)]
pub enum WorkshopError {
    RecipeNotFound,
    NotUnlocked,
    RegionNotFound,
    InventoryMissing,
    InsufficientIngredients,
}

fn try_consume_from_inventory(
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
