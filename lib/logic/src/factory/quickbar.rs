//! Quickbar business logic.
//!
//! The quickbar holds 7 types of item shortcuts. `set_one` assigns an
//! item to a slot, `move_one` swaps two slots within the same type.

impl crate::factory::FactoryManager {
    /// Set a single quickbar slot. `qb_type` is the quickbar type ID,
    /// `index` is the slot, `item_id` is the template to assign.
    /// An empty `item_id` clears the slot.
    pub fn quickbar_set_one(&mut self, qb_type: i32, index: i32, item_id: &str) -> bool {
        if index < 0 {
            return false;
        }
        let idx = index as usize;

        // Find or create the quickbar for this type.
        let qb = self
            .quickbars
            .iter_mut()
            .find(|q| q.quickbar_type == qb_type.to_string());

        if let Some(qb) = qb {
            // Grow the list if needed.
            if idx >= qb.items.len() {
                qb.items.resize(idx + 1, String::new());
            }
            qb.items[idx] = item_id.to_string();
        } else {
            // Create a new quickbar entry.
            let mut items = vec![String::new(); idx];
            items.push(item_id.to_string());
            self.quickbars.push(crate::factory::QuickbarState {
                quickbar_type: qb_type.to_string(),
                items,
            });
        }

        true
    }

    /// Move an item from one slot to another within the same quickbar
    /// type. Swaps the two slots.
    pub fn quickbar_move_one(&mut self, qb_type: i32, from_index: i32, to_index: i32) -> bool {
        if from_index < 0 || to_index < 0 {
            return false;
        }
        let from = from_index as usize;
        let to = to_index as usize;

        let Some(qb) = self
            .quickbars
            .iter_mut()
            .find(|q| q.quickbar_type == qb_type.to_string())
        else {
            return false;
        };

        if from >= qb.items.len() || to >= qb.items.len() {
            // Grow to accommodate the larger index.
            let max = from.max(to);
            qb.items.resize(max + 1, String::new());
        }

        qb.items.swap(from, to);
        true
    }
}
