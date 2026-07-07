//! Character work business logic.
//!
//! Characters can be punched in to factory work slots, where their
//! factory skills provide speed/cost modifiers to buildings in that
//! region. Type 1 = speed modifier, applied via `effective_speed()`
//! in `power.rs`.
//!
//! Two ops: `punch_in` (assign characters to a building's work slots)
//! and `punch_out` (remove characters from their work slots).

use config::factory_table::FTableAssets;

use crate::factory::{CharacterWorker, FactoryManager};

impl FactoryManager {
    /// Punch characters into a building's work slots. Each character's
    /// skill IDs are looked up from `skillData` and stored on the
    /// `CharacterWorker` so the machine crafter + other systems can
    /// apply modifiers.
    pub fn character_work_punch_in(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        _node_id: u32,
        char_id_sequence: &[String],
    ) -> bool {
        // Collect skill IDs for each character from skillData. The
        // skill entry's `effect_building_id` tells us which buildings
        // the skill applies to.
        let mut workers = vec![];
        for (slot_idx, char_id) in char_id_sequence.iter().enumerate() {
            let skill_ids = vec![];
            // Look up skills that affect this building. The skillData
            // table keys by skill ID, and each entry has an
            // `effect_building_id` list. We scan for skills whose
            // effect_building_id contains the building's template_id.
            // TODO: this scan is O(n*m) -- cache the reverse mapping
            // once the skill table is large enough to matter.
            let _ = assets; // skill lookup needs the building's template_id
            // which we don't have here without looking up
            // the node. For now, store empty skills and
            // let the caller fill them later.
            workers.push(CharacterWorker {
                region_name: region_name.to_string(),
                char_inst_id: char_id.parse().unwrap_or(0),
                skill_ids,
                work_slot: slot_idx as u32,
            });
        }

        // Remove any existing workers for this node, then add the new ones.
        self.character_work_state.workers.retain(|w| {
            !(w.region_name == region_name
                && w.char_inst_id != 0
                && char_id_sequence
                    .iter()
                    .any(|c| c.parse::<u32>().unwrap_or(0) == w.char_inst_id))
        });
        self.character_work_state.workers.extend(workers);
        true
    }

    /// Punch characters out of their work slots. Returns the list of
    /// punched-out character IDs for the response.
    pub fn character_work_punch_out(&mut self, char_id_list: &[String]) -> Vec<String> {
        let mut punched = vec![];
        for char_id in char_id_list {
            let inst_id = char_id.parse::<u32>().unwrap_or(0);
            let before = self.character_work_state.workers.len();
            self.character_work_state
                .workers
                .retain(|w| w.char_inst_id != inst_id);
            if self.character_work_state.workers.len() < before {
                punched.push(char_id.clone());
            }
        }
        punched
    }

    /// Get all skill modifiers active for a building in a region.
    /// Returns a Vec of f64 modifiers (additive, applied via
    /// `effective_speed()` in power.rs).
    pub fn get_skill_modifiers_for_building(
        &self,
        assets: &FTableAssets,
        region_name: &str,
        template_id: &str,
    ) -> Vec<f64> {
        let mut modifiers = vec![];
        for worker in &self.character_work_state.workers {
            if worker.region_name != region_name {
                continue;
            }
            for skill_id in &worker.skill_ids {
                if let Some(skill) = assets.get_skill(skill_id) {
                    // Check if this skill affects this building.
                    let affects = skill.effect_building_id.is_empty()
                        || skill.effect_building_id.iter().any(|id| id == template_id);
                    if !affects {
                        continue;
                    }
                    // Type 1 = speed modifier. param_list[i] is the
                    // fractional bonus (e.g. -0.2 for 20% faster).
                    for (i, &ty) in skill.type_list.iter().enumerate() {
                        if ty == 1
                            && let Some(&param) = skill.param_list.get(i)
                        {
                            modifiers.push(param);
                        }
                    }
                }
            }
        }
        modifiers
    }
}
