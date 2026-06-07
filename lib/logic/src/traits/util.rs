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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{WeaponDepot, WeaponInstance};

    #[test]
    fn count_by_template_empty() {
        let items: Vec<WeaponInstance> = vec![];
        let counts = count_by_template(&items);
        assert!(counts.is_empty());
    }

    #[test]
    fn count_by_template_single_item() {
        let mut depot = WeaponDepot::new();
        depot.add_weapon("wpn_sword".to_string(), 0);
        let weapons: Vec<&WeaponInstance> = depot.all_weapons().values().collect();
        let counts = count_by_template(weapons.iter().copied());
        assert_eq!(counts.len(), 1);
        assert_eq!(counts["wpn_sword"], 1);
    }

    #[test]
    fn count_by_template_multiple_same_template() {
        let mut depot = WeaponDepot::new();
        depot.add_weapon("wpn_sword".to_string(), 0);
        depot.add_weapon("wpn_sword".to_string(), 0);
        depot.add_weapon("wpn_sword".to_string(), 0);
        let weapons: Vec<&WeaponInstance> = depot.all_weapons().values().collect();
        let counts = count_by_template(weapons.iter().copied());
        assert_eq!(counts.len(), 1);
        assert_eq!(counts["wpn_sword"], 3);
    }

    #[test]
    fn count_by_template_mixed_templates() {
        let mut depot = WeaponDepot::new();
        depot.add_weapon("wpn_sword".to_string(), 0);
        depot.add_weapon("wpn_bow".to_string(), 0);
        depot.add_weapon("wpn_sword".to_string(), 0);
        let weapons: Vec<&WeaponInstance> = depot.all_weapons().values().collect();
        let counts = count_by_template(weapons.iter().copied());
        assert_eq!(counts.len(), 2);
        assert_eq!(counts["wpn_sword"], 2);
        assert_eq!(counts["wpn_bow"], 1);
    }
}
