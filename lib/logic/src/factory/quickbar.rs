//! Quickbar business logic.
//!
//! The quickbar holds 2 pages of item shortcuts (`FCQuickBarType`:
//! `Inner`=0, `Outer`=1), each a flat 4x8 grid of 32 slots (row-major:
//! `index = barIndex * 8 + slotIndex`). `set_one` assigns an item to a
//! slot, `move_one` swaps two slots within the same page.

impl crate::factory::FactoryManager {
    /// Set a single quickbar slot. `qb_type` is the `FCQuickBarType`
    /// (`Inner`=0, `Outer`=1), `index` is the flat slot (0..=31),
    /// `item_id` is the template to assign. An empty `item_id` clears
    /// the slot.
    pub fn quickbar_set_one(&mut self, qb_type: i32, index: i32, item_id: &str) -> bool {
        if !(0..crate::factory::QUICKBAR_SIZE as i32).contains(&index) {
            return false;
        }
        let idx = index as usize;

        // Find or create the quickbar for this type.
        let qb = self
            .quickbars
            .iter_mut()
            .find(|q| q.quickbar_type == qb_type);

        if let Some(qb) = qb {
            qb.items[idx] = item_id.to_string();
        } else {
            let mut items = vec![String::new(); crate::factory::QUICKBAR_SIZE];
            items[idx] = item_id.to_string();
            self.quickbars.push(crate::factory::QuickbarState {
                quickbar_type: qb_type,
                items,
            });
        }

        true
    }

    /// Move an item from one slot to another within the same quickbar
    /// page.
    pub fn quickbar_move_one(&mut self, qb_type: i32, from_index: i32, to_index: i32) -> bool {
        if !(0..crate::factory::QUICKBAR_SIZE as i32).contains(&from_index)
            || !(0..crate::factory::QUICKBAR_SIZE as i32).contains(&to_index)
        {
            return false;
        }
        let from = from_index as usize;
        let to = to_index as usize;

        let Some(qb) = self
            .quickbars
            .iter_mut()
            .find(|q| q.quickbar_type == qb_type)
        else {
            return false;
        };

        qb.items.swap(from, to);
        true
    }
}
