use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// Beyond.GEnums.BitsetType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BitsetType {
    None = 0,
    FoundItem = 1,
    Wiki = 2,
    UnreadWiki = 3,
    MonsterDrop = 4,
    GotItem = 5,
    AreaFirstView = 6,
    UnreadGotItem = 7,
    Prts = 8,
    UnreadPrts = 9,
    PrtsFirstLv = 10,
    PrtsTerminalContent = 11,
    LevelHaveBeen = 12,
    LevelMapFirstView = 13,
    UnreadFormula = 14,
    NewChar = 15,
    ElogChannel = 16,
    FmvWatched = 17,
    TimeLineWatched = 18,
    MapFilter = 19,
    EnumMax = 20,
}

impl BitsetType {
    pub fn from_i32(val: i32) -> Option<Self> {
        match val {
            0 => Some(Self::None),
            1 => Some(Self::FoundItem),
            2 => Some(Self::Wiki),
            3 => Some(Self::UnreadWiki),
            4 => Some(Self::MonsterDrop),
            5 => Some(Self::GotItem),
            6 => Some(Self::AreaFirstView),
            7 => Some(Self::UnreadGotItem),
            8 => Some(Self::Prts),
            9 => Some(Self::UnreadPrts),
            10 => Some(Self::PrtsFirstLv),
            11 => Some(Self::PrtsTerminalContent),
            12 => Some(Self::LevelHaveBeen),
            13 => Some(Self::LevelMapFirstView),
            14 => Some(Self::UnreadFormula),
            15 => Some(Self::NewChar),
            16 => Some(Self::ElogChannel),
            17 => Some(Self::FmvWatched),
            18 => Some(Self::TimeLineWatched),
            19 => Some(Self::MapFilter),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BitsetManager {
    bits: HashMap<BitsetType, HashSet<u32>>,
}

macro_rules! bitset_helpers {
    ($(
        $variant:ident => $mark:ident / $has:ident ;
    )*) => {
        $(
            pub fn $mark(&mut self, id: u32) {
                self.set(BitsetType::$variant, id);
            }

            pub fn $has(&self, id: u32) -> bool {
                self.has(BitsetType::$variant, id)
            }
        )*
    };
}

impl BitsetManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, bitset_type: BitsetType, bit: u32) {
        self.bits.entry(bitset_type).or_default().insert(bit);
    }

    pub fn unset(&mut self, bitset_type: BitsetType, bit: u32) {
        if let Some(set) = self.bits.get_mut(&bitset_type) {
            set.remove(&bit);
        }
    }

    pub fn unset_many(&mut self, bitset_type: BitsetType, bits: &[u32]) {
        if let Some(set) = self.bits.get_mut(&bitset_type) {
            for &bit in bits {
                set.remove(&bit);
            }
        }
    }

    pub fn has(&self, bitset_type: BitsetType, bit: u32) -> bool {
        self.bits
            .get(&bitset_type)
            .map(|s| s.contains(&bit))
            .unwrap_or(false)
    }

    pub fn get_bits(&self, bitset_type: BitsetType) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .bits
            .get(&bitset_type)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default();
        v.sort_unstable();
        v
    }

    pub fn count(&self, bitset_type: BitsetType) -> usize {
        self.bits.get(&bitset_type).map(|s| s.len()).unwrap_or(0)
    }

    bitset_helpers! {
        FoundItem        => mark_item_found / has_item_found;
        Wiki             => mark_wiki / has_wiki;
        UnreadWiki       => mark_unread_wiki / has_unread_wiki;
        MonsterDrop      => mark_monster_drop / has_monster_drop;
        GotItem          => mark_got_item / has_got_item;
        AreaFirstView    => mark_area_visited / has_visited_area;
        UnreadGotItem    => mark_unread_got_item / has_unread_got_item;
        Prts             => mark_prts / has_prts;
        UnreadPrts       => mark_unread_prts / has_unread_prts;
        PrtsFirstLv      => mark_prts_first_lv / has_prts_first_lv;
        PrtsTerminalContent => mark_prts_terminal_content / has_prts_terminal_content;
        LevelHaveBeen    => mark_level_visited / has_visited_level;
        LevelMapFirstView => mark_level_map_first_view / has_level_map_first_view;
        UnreadFormula    => mark_unread_formula / has_unread_formula;
        NewChar          => mark_new_char / has_new_char;
        ElogChannel      => mark_elog_channel / has_elog_channel;
        FmvWatched       => mark_fmv_watched / has_fmv_watched;
        TimeLineWatched  => mark_timeline_watched / has_timeline_watched;
        MapFilter        => mark_map_filter / has_map_filter;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_i32_known_values() {
        assert_eq!(BitsetType::from_i32(0), Some(BitsetType::None));
        assert_eq!(BitsetType::from_i32(1), Some(BitsetType::FoundItem));
        assert_eq!(BitsetType::from_i32(5), Some(BitsetType::GotItem));
        assert_eq!(BitsetType::from_i32(19), Some(BitsetType::MapFilter));
    }

    #[test]
    fn from_i32_unknown_returns_none() {
        assert_eq!(BitsetType::from_i32(100), None);
        assert_eq!(BitsetType::from_i32(-1), None);
    }

    #[test]
    fn from_i32_enum_max_is_unknown() {
        // EnumMax = 20 is not a valid variant for from_i32
        assert_eq!(BitsetType::from_i32(20), None);
    }

    #[test]
    fn new_manager_is_empty() {
        let mgr = BitsetManager::new();
        assert!(!mgr.has(BitsetType::FoundItem, 1));
        assert!(!mgr.has(BitsetType::Wiki, 42));
        assert_eq!(mgr.count(BitsetType::FoundItem), 0);
        assert_eq!(mgr.get_bits(BitsetType::FoundItem), Vec::<u32>::new());
    }

    #[test]
    fn set_and_has() {
        let mut mgr = BitsetManager::new();
        mgr.set(BitsetType::FoundItem, 10);
        assert!(mgr.has(BitsetType::FoundItem, 10));
        assert!(!mgr.has(BitsetType::FoundItem, 11));
        assert!(!mgr.has(BitsetType::Wiki, 10));
    }

    #[test]
    fn unset_removes_bit() {
        let mut mgr = BitsetManager::new();
        mgr.set(BitsetType::FoundItem, 10);
        assert!(mgr.has(BitsetType::FoundItem, 10));
        mgr.unset(BitsetType::FoundItem, 10);
        assert!(!mgr.has(BitsetType::FoundItem, 10));
    }

    #[test]
    fn unset_many_removes_multiple() {
        let mut mgr = BitsetManager::new();
        mgr.set(BitsetType::Wiki, 1);
        mgr.set(BitsetType::Wiki, 2);
        mgr.set(BitsetType::Wiki, 3);
        mgr.unset_many(BitsetType::Wiki, &[1, 3]);
        assert!(!mgr.has(BitsetType::Wiki, 1));
        assert!(mgr.has(BitsetType::Wiki, 2));
        assert!(!mgr.has(BitsetType::Wiki, 3));
    }

    #[test]
    fn unset_nonexistent_is_noop() {
        let mut mgr = BitsetManager::new();
        // Should not panic
        mgr.unset(BitsetType::None, 999);
    }

    #[test]
    fn count_tracks_entries() {
        let mut mgr = BitsetManager::new();
        assert_eq!(mgr.count(BitsetType::GotItem), 0);
        mgr.set(BitsetType::GotItem, 1);
        mgr.set(BitsetType::GotItem, 2);
        mgr.set(BitsetType::GotItem, 5);
        assert_eq!(mgr.count(BitsetType::GotItem), 3);
    }

    #[test]
    fn get_bits_returns_sorted() {
        let mut mgr = BitsetManager::new();
        mgr.set(BitsetType::Prts, 5);
        mgr.set(BitsetType::Prts, 1);
        mgr.set(BitsetType::Prts, 3);
        let bits = mgr.get_bits(BitsetType::Prts);
        assert_eq!(bits, vec![1, 3, 5]);
    }

    #[test]
    fn set_same_id_idempotent() {
        let mut mgr = BitsetManager::new();
        mgr.set(BitsetType::NewChar, 7);
        mgr.set(BitsetType::NewChar, 7);
        assert_eq!(mgr.count(BitsetType::NewChar), 1);
    }

    #[test]
    fn macro_helpers_work() {
        let mut mgr = BitsetManager::new();
        mgr.mark_item_found(42);
        assert!(mgr.has_item_found(42));
        mgr.mark_wiki(100);
        assert!(mgr.has_wiki(100));
        mgr.mark_fmv_watched(5);
        assert!(mgr.has_fmv_watched(5));
        // Cross-type: FMV watched should not show up as found item
        assert!(!mgr.has_item_found(5));
    }

    #[test]
    fn serialization_roundtrip() {
        let mut mgr = BitsetManager::new();
        mgr.set(BitsetType::FoundItem, 1);
        mgr.set(BitsetType::FoundItem, 2);
        mgr.set(BitsetType::Wiki, 10);
        let json = serde_json::to_string(&mgr).unwrap();
        let decoded: BitsetManager = serde_json::from_str(&json).unwrap();
        assert!(decoded.has(BitsetType::FoundItem, 1));
        assert!(decoded.has(BitsetType::FoundItem, 2));
        assert!(decoded.has(BitsetType::Wiki, 10));
        assert!(!decoded.has(BitsetType::Wiki, 11));
    }
}
