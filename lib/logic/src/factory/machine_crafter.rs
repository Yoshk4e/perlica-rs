//! Machine crafter runtime -- the completion checker.
//!
//! Every tick (or on-demand), the checker scans every Producer component
//! in every region. For each one with an active `formula_id` and
//! `start_tick`, it computes elapsed progress and checks if the recipe's
//! `total_progress` has been reached. If so, it:
//!
//! 1. Produces the outcome items into the producer's outcome Cache.
//! 2. Consumes the ingredient items from the producer's ingredient Cache.
//! 3. Resets `start_tick` to the current tick (auto-restart if signal
//!    is still active and ingredients are still available).
//!
//! The speed comes from `machineCrafterData[id].speed` (default 100).
//! The recipe comes from `machineCraftData[formula_id]`.

use config::factory_table::FTableAssets;
use config::tables::factory_table::{ItemCount, ItemCountGroup};

use crate::factory::tick::{Tick, current_tick, elapsed_since};
use crate::factory::{FactoryComponent, FactoryManager, FactoryRegion, ItemSlot};

impl FactoryManager {
    /// Run one completion-check pass across every region. Called
    /// periodically by the server's tick loop (or on-demand before
    /// sending an HsFb response so progress is up to date).
    ///
    /// Returns the number of recipes that completed this pass.
    pub fn tick_machine_crafters(&mut self, assets: &FTableAssets) -> usize {
        let now = current_tick();
        let mut completed = 0;

        for region in self.regions.values_mut() {
            completed += tick_region(region, assets, now);
        }

        completed
    }
}

fn tick_region(region: &mut FactoryRegion, assets: &FTableAssets, now: Tick) -> usize {
    let mut completed = 0;

    // Collect node IDs that have a Producer component, then process them.
    // We collect first to avoid holding a borrow on `region.nodes` while
    // we mutate component state.
    let producer_node_ids: Vec<u32> = region
        .nodes
        .values()
        .filter(|n| {
            n.components
                .iter()
                .any(|(_, c)| matches!(c, FactoryComponent::Producer(s) if !s.formula_id.is_empty()))
        })
        .map(|n| n.node_id)
        .collect();

    for node_id in producer_node_ids {
        let Some(node) = region.node_mut(node_id) else {
            continue;
        };

        // Find the Producer component and its formula_id + start_tick.
        let (formula_id, start_tick, speed) = {
            let mut found = None;
            for (_, comp) in &node.components {
                if let FactoryComponent::Producer(state) = comp
                    && !state.formula_id.is_empty() && state.start_tick.is_some() {
                        // Look up speed from machineCrafterData using the
                        // node's template_id (which is the machine_id).
                        let speed = assets
                            .get_machine_crafter(&node.template_id)
                            .map_or(100, |mc| mc.speed);
                        found = Some((state.formula_id.clone(), state.start_tick, speed));
                        break;
                    }
            }
            match found {
                Some(v) => v,
                None => continue,
            }
        };

        // Look up the recipe.
        let Some(recipe) = assets.get_machine_craft(&formula_id).cloned() else {
            continue;
        };

        let Some(start) = start_tick else {
            continue;
        };

        let elapsed = elapsed_since(start);
        let progress = elapsed.saturating_mul(speed);

        if progress < recipe.total_progress {
            continue;
        }

        // Recipe complete! Consume ingredients from the node's Cache
        // components and produce outcomes.
        //
        // The ingredient Cache is the one wired to CacheIn1..CacheIn4
        // via the machine's `ingredient_buffer_binding`. The outcome
        // Cache is wired to CacheOut1..CacheOut4 via
        // `outcome_buffer_binding`. For now we just look for any Cache
        // component on this node.
        let machine_crafter = assets.get_machine_crafter(&node.template_id).cloned();

        // Consume ingredients.
        let ingredients_ok = consume_ingredients(node, &recipe.ingredients);

        if !ingredients_ok {
            // Not enough ingredients -- pause the producer.
            if let Some(slot) = node.component_mut(find_producer_id(node))
                && let FactoryComponent::Producer(state) = slot {
                    state.start_tick = None;
                }
            continue;
        }

        // Produce outcomes.
        produce_outcomes(node, &recipe.outcomes);

        // Restart the recipe.
        if let Some(slot) = node.component_mut(find_producer_id(node))
            && let FactoryComponent::Producer(state) = slot {
                state.last_formula_id = state.formula_id.clone();
                state.current_progress = 0;
                state.start_tick = Some(now);
            }

        // Record the completed recipe in production_totals for STT.
        // Collect the outcome items first to avoid holding a borrow on
        // `node` while we mutate `region.production_totals`.
        let mut produced: Vec<(String, u64)> = vec![];
        for outcome in &recipe.outcomes {
            for item in &outcome.group {
                produced.push((item.id.clone(), item.count as u64));
            }
        }
        for (id, count) in produced {
            *region.production_totals.entry(id).or_insert(0) += count;
        }

        let _ = machine_crafter;
        completed += 1;
    }

    completed
}

/// Try to consume one unit of each ingredient group from the node's
/// Cache components. Each `ItemCountGroup` is a "pick one of these"
/// list -- we take the first matching item that has enough count.
fn consume_ingredients(
    node: &mut crate::factory::FactoryNode,
    ingredient_groups: &[ItemCountGroup],
) -> bool {
    // First pass: check that every group has at least one available item.
    for group in ingredient_groups {
        if !has_any_ingredient(node, &group.group) {
            return false;
        }
    }

    // Second pass: actually consume.
    for group in ingredient_groups {
        if !take_any_ingredient(node, &group.group) {
            return false;
        }
    }

    true
}

fn has_any_ingredient(node: &crate::factory::FactoryNode, items: &[ItemCount]) -> bool {
    for (_, comp) in &node.components {
        if let FactoryComponent::Cache(state) = comp {
            for slot in &state.items {
                for item in items {
                    if slot.item_id == item.id && slot.count >= item.count {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn take_any_ingredient(node: &mut crate::factory::FactoryNode, items: &[ItemCount]) -> bool {
    for (_, comp) in &mut node.components {
        if let FactoryComponent::Cache(state) = comp {
            for slot in &mut state.items {
                for item in items {
                    if slot.item_id == item.id && slot.count >= item.count {
                        slot.count -= item.count;
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Push outcome items into the node's Cache components. Stacks with
/// existing same-item slots, or pushes a new slot if full/empty.
fn produce_outcomes(node: &mut crate::factory::FactoryNode, outcome_groups: &[ItemCountGroup]) {
    for group in outcome_groups {
        for item in &group.group {
            let mut placed = false;
            for (_, comp) in &mut node.components {
                if let FactoryComponent::Cache(state) = comp {
                    for slot in &mut state.items {
                        if slot.item_id == item.id && slot.count > 0 {
                            slot.count += item.count;
                            placed = true;
                            break;
                        }
                    }
                    if placed {
                        break;
                    }
                }
            }
            if !placed {
                for (_, comp) in &mut node.components {
                    if let FactoryComponent::Cache(state) = comp {
                        state.items.push(ItemSlot {
                            item_id: item.id.clone(),
                            count: item.count,
                            inst_id: 0,
                        });
                        placed = true;
                        break;
                    }
                }
            }
            let _ = placed;
        }
    }
}

fn find_producer_id(node: &crate::factory::FactoryNode) -> u32 {
    for (id, comp) in &node.components {
        if matches!(comp, FactoryComponent::Producer(_)) {
            return *id;
        }
    }
    0
}
