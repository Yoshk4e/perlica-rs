//! Standalone generic helpers that don't fit any other module.

use std::collections::HashMap;

use crate::traits::item::Templated;

/// Count how many items of each distinct template id appear in `iter`.
pub fn count_by_template<'a, I, T>(iter: I) -> HashMap<String, u32>
where
    I: IntoIterator<Item = &'a T>,
    T: Templated + 'a,
{
    let mut map: HashMap<String, u32> = HashMap::new();
    for item in iter {
        *map.entry(item.template_id().to_owned()).or_insert(0) += 1;
    }
    map
}
