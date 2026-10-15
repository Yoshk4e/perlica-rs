//! Manual work business logic.
//!
//! Manual crafting is a player-driven queue of at most
//! `manualCraftQueueLength = 5` stacks, each folding up to 10 identical crafts
//! (`ManualWorkUnit.count`). A craft takes `totalProgress = 120` ticks at speed
//! 1 (2 minutes). Only the head stack's current craft accrues progress, and the
//! queue drains lazily -- there is no tick loop.
//!
//! Four ops: `append` (add to queue), `cancel` (clear + return
//! ingredients), `pause`, `resume`.

use config::factory_table::FTableAssets;

use crate::factory::{FactoryManager, Tick, current_tick, push_item_to_bag, take_bill_from_inv};

/// Progress per tick for a manual craft.
///
/// Manual crafting has no speed modifier — a recipe's `totalProgress` is its
/// duration in ticks directly. Named rather than inlined so the day a skill or
/// building level speeds it up, there is one place to change.
const MANUAL_WORK_SPEED: u64 = 1;

impl FactoryManager {
    /// Append manual crafts to the queue, folding into the tail stack where
    /// possible. Consumes ingredients for the crafts that were admitted.
    ///
    /// Returns how many crafts were queued: `0` if the recipe is unknown, the
    /// queue is full, or the bag cannot pay. A partially-fitting request is
    /// clamped rather than rejected, and only the admitted crafts are charged —
    /// the old code checked the queue cap *before* pushing `count` units, so
    /// appending 5 onto a queue of 4 produced 9.
    pub fn manual_work_append(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        formula_id: &str,
        count: i32,
    ) -> u32 {
        let Some(recipe) = assets.get_manual_craft(formula_id) else {
            return 0;
        };

        let want = count.max(1) as u32;
        let admitted = want.min(self.manual_work_state.room_for(formula_id));
        if admitted == 0 {
            return 0;
        }

        // The whole bill, priced for the admitted crafts. Built before the bag
        // borrow so a short bag costs the player nothing.
        let bill: Vec<(&str, u32)> = recipe
            .ingredients
            .iter()
            .map(|ing| (ing.id.as_str(), ing.count.saturating_mul(admitted)))
            .collect();

        let Some(region) = self.region_mut(region_name) else {
            return 0;
        };
        let Some(bag_node) = region.node_mut(1) else {
            return 0;
        };
        let Some(bag_comp) = bag_node.component_mut(1) else {
            return 0;
        };
        let crate::factory::FactoryComponent::Inventory(bag_inv) = bag_comp else {
            return 0;
        };

        if !take_bill_from_inv(bag_inv, &bill) {
            return 0;
        }

        self.manual_work_state
            .append_at(current_tick(), formula_id, admitted, MANUAL_WORK_SPEED)
    }

    /// Cancel the manual work queue. Returns the ingredients to refund
    /// and the items that would have broken (lost).
    ///
    /// The in-flight craft is refunded in full rather than pro-rated: the
    /// player gets their materials back, which is the forgiving reading and the
    /// one that cannot lose items to rounding.
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

        // Refund every queued craft, not just one per stack.
        for unit in &self.manual_work_state.queue {
            if let Some(recipe) = assets.get_manual_craft(&unit.recipe_id) {
                for ingredient in &recipe.ingredients {
                    let refund = ingredient.count.saturating_mul(unit.count);
                    *back_items.entry(ingredient.id.clone()).or_insert(0) +=
                        i32::try_from(refund).unwrap_or(i32::MAX);
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
                push_item_to_bag(inv_state, item_id, (*count).max(0) as u32, 0);
            }
        }

        self.manual_work_state.queue.clear();
        self.manual_work_state.paused_at = None;

        (back_items, break_items)
    }

    /// Pause the manual work queue, freezing the head unit's progress.
    pub fn manual_work_pause(&mut self) {
        self.manual_work_state.pause_at(current_tick());
    }

    /// Resume the manual work queue; the head continues from where it froze.
    pub fn manual_work_resume(&mut self) {
        self.manual_work_state.resume_at(current_tick());
    }

    /// Whether the queue is halted rather than working. This is the wire's
    /// `in_block`.
    ///
    /// It deliberately reports the player pause too: `in_block` was the only
    /// halt signal the client had before `pause_tms` existed, so narrowing it
    /// to non-pause stalls would silently un-halt the UI. `pause_tms` is what
    /// distinguishes "player paused" from "cannot progress".
    pub fn manual_work_in_block(&self) -> bool {
        if self.manual_work_state.is_paused() {
            return true;
        }
        self.manual_work_state
            .queue
            .first()
            .is_some_and(|head| head.timing.speed == 0)
    }

    /// Settle the manual work queue: complete every craft whose progress has
    /// reached its recipe's `totalProgress` and push the outcomes to the bag.
    ///
    /// Lazy catch-up, like the rest of the factory: there is no per-second
    /// tick loop, so this drains however many crafts the elapsed time covers.
    /// Each completion chains the next craft from the *previous* craft's real
    /// completion instant, not from now — otherwise a queue left overnight
    /// would finish only its head.
    ///
    /// Returns the number of crafts completed.
    pub fn tick_manual_work(&mut self, assets: &FTableAssets, region_name: &str) -> usize {
        if self.manual_work_state.is_paused() {
            return 0;
        }

        /// What to do with the stack currently at the head of the queue.
        enum Step {
            /// A craft finished at this tick; grant outcomes and chain the next.
            Settle { recipe_id: String, done_at: Tick },
            /// Its recipe is gone from config; discard the whole stack.
            Discard,
            /// Still in flight (or frozen): stop settling.
            Wait,
        }

        let now = current_tick();
        let mut completed = 0;

        loop {
            let step = {
                let Some(head) = self.manual_work_state.queue.first() else {
                    break;
                };
                match assets.get_manual_craft(&head.recipe_id) {
                    None => Step::Discard,
                    // `scheduled_completion` is the historical instant, so it
                    // is also the tick the next craft starts from.
                    Some(recipe) => match head.timing.scheduled_completion(recipe.total_progress) {
                        Some(done_at) if done_at <= now => Step::Settle {
                            recipe_id: head.recipe_id.clone(),
                            done_at,
                        },
                        _ => Step::Wait,
                    },
                }
            };

            let (done_at, drop_stack) = match step {
                Step::Wait => break,
                // The recipe vanished from config: nothing in this stack can
                // ever be made, so the whole stack goes.
                Step::Discard => (now, true),
                Step::Settle { recipe_id, done_at } => {
                    self.grant_manual_outcomes(assets, region_name, &recipe_id);
                    completed += 1;

                    // One craft off the head stack; the stack itself only
                    // leaves the queue once it is empty.
                    let emptied = match self.manual_work_state.queue.first_mut() {
                        Some(head) => {
                            head.count = head.count.saturating_sub(1);
                            head.count == 0
                        }
                        None => true,
                    };
                    (done_at, emptied)
                }
            };

            if drop_stack {
                self.manual_work_state.queue.remove(0);
            }
            // Whether the head is a new stack or the next craft of the same
            // one, it starts from the previous craft's real completion instant.
            if let Some(head) = self.manual_work_state.queue.first_mut() {
                head.timing.restart_at(done_at);
            }
            self.manual_work_state.sync_head_at(done_at);
        }

        completed
    }

    /// Push one manual recipe's outcomes into the factory bag.
    fn grant_manual_outcomes(&mut self, assets: &FTableAssets, region_name: &str, recipe_id: &str) {
        let Some(recipe) = assets.get_manual_craft(recipe_id) else {
            return;
        };
        let Some(region) = self.region_mut(region_name) else {
            return;
        };
        if let Some(bag_node) = region.node_mut(1)
            && let Some(bag_comp) = bag_node.component_mut(1)
            && let crate::factory::FactoryComponent::Inventory(inv_state) = bag_comp
        {
            for outcome in &recipe.outcomes {
                push_item_to_bag(inv_state, &outcome.id, outcome.count, 0);
            }
        }
    }

    /// Virtual start of the head craft as of `now`, for the wire's
    /// `head_start_tms`: the instant a client can extrapolate progress from.
    pub fn manual_work_head_start(&self, now: Tick) -> Tick {
        self.manual_work_state
            .queue
            .first()
            .map_or(0, |unit| unit.timing.effective_start_at(now))
    }
}
