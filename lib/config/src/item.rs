use crate::error::{ConfigError, Result};
use crate::tables::item::{I18nText, ItemFile, RawItemEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u32)]
pub enum CraftShowingType {
    #[default]
    None = 0,
    SourceMachine = 1,
    BasicMachine = 2,
    AssembleMachine = 3,
    FarmMachine = 4,
    UtilMachine = 5,
    ExpCard = 6,
    EquipHead = 7,
    EquipBody = 8,
    EquipRing = 9,
    RankupMaterial = 10,
    ManualCraftFactory = 11,
    ManualCraftEquip = 12,
    ManualCraftMedic = 13,
    ExpCardProc = 14,
    WeaponGemNormal = 15,
    WeaponGemSpc = 16,
}

impl CraftShowingType {
    #[inline]
    pub fn is_equip_slot(self) -> bool {
        matches!(self, Self::EquipHead | Self::EquipBody | Self::EquipRing)
    }
}

impl TryFrom<u32> for CraftShowingType {
    type Error = u32;
    fn try_from(v: u32) -> std::result::Result<Self, u32> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::SourceMachine),
            2 => Ok(Self::BasicMachine),
            3 => Ok(Self::AssembleMachine),
            4 => Ok(Self::FarmMachine),
            5 => Ok(Self::UtilMachine),
            6 => Ok(Self::ExpCard),
            7 => Ok(Self::EquipHead),
            8 => Ok(Self::EquipBody),
            9 => Ok(Self::EquipRing),
            10 => Ok(Self::RankupMaterial),
            11 => Ok(Self::ManualCraftFactory),
            12 => Ok(Self::ManualCraftEquip),
            13 => Ok(Self::ManualCraftMedic),
            14 => Ok(Self::ExpCardProc),
            15 => Ok(Self::WeaponGemNormal),
            16 => Ok(Self::WeaponGemSpc),
            other => Err(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum ItemDepotType {
    Invalid = 0,
    Weapon = 1,
    WeaponGem = 2,
    Equip = 3,
    SpecialItem = 4,
    MissionItem = 5,
    Factory = 6,
}

impl ItemDepotType {
    pub const ALL_VALID: &'static [Self] = &[
        Self::Weapon,
        Self::WeaponGem,
        Self::Equip,
        Self::SpecialItem,
        Self::MissionItem,
        Self::Factory,
    ];
}

impl TryFrom<u32> for ItemDepotType {
    type Error = u32;
    fn try_from(v: u32) -> std::result::Result<Self, u32> {
        match v {
            0 => Ok(Self::Invalid),
            1 => Ok(Self::Weapon),
            2 => Ok(Self::WeaponGem),
            3 => Ok(Self::Equip),
            4 => Ok(Self::SpecialItem),
            5 => Ok(Self::MissionItem),
            6 => Ok(Self::Factory),
            other => Err(other),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemKind {
    Weapon,
    WeaponGem { craft_slot: CraftShowingType },
    Equip { slot: CraftShowingType },
    SpecialItem,
    MissionItem,
    Factory,
    Unknown { raw_tab_type: u32 },
}

impl ItemKind {
    #[inline]
    pub fn is_instanced(&self) -> bool {
        matches!(
            self,
            Self::Weapon | Self::WeaponGem { .. } | Self::Equip { .. }
        )
    }

    #[inline]
    pub fn is_stackable(&self) -> bool {
        matches!(self, Self::SpecialItem | Self::MissionItem | Self::Factory)
    }

    pub fn depot_type(&self) -> ItemDepotType {
        match self {
            Self::Weapon => ItemDepotType::Weapon,
            Self::WeaponGem { .. } => ItemDepotType::WeaponGem,
            Self::Equip { .. } => ItemDepotType::Equip,
            Self::SpecialItem => ItemDepotType::SpecialItem,
            Self::MissionItem => ItemDepotType::MissionItem,
            Self::Factory => ItemDepotType::Factory,
            Self::Unknown { .. } => ItemDepotType::Invalid,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemConfig {
    pub id: String,
    pub item_type: u32,
    pub name: I18nText,
    pub desc: I18nText,
    pub rarity: u32,
    pub sort_id1: i32,
    pub sort_id2: i32,
    pub icon_id: String,
    pub model_key: String,
    /// -1 = unlimited.
    pub max_backpack_stack_count: i32,
    /// -1 = unlimited; 1 = non-stackable / instanced.
    pub max_stack_count: i32,
    pub can_discard: bool,
    pub price: u64,
    pub obtain_way_ids: Vec<String>,
    pub depot_type: ItemDepotType,
    pub kind: ItemKind,
}

impl ItemConfig {
    #[inline]
    pub fn is_instanced(&self) -> bool {
        self.kind.is_instanced()
    }
    #[inline]
    pub fn is_stackable(&self) -> bool {
        self.kind.is_stackable()
    }
}

impl From<RawItemEntry> for ItemConfig {
    fn from(r: RawItemEntry) -> Self {
        let depot_type =
            ItemDepotType::try_from(r.valuable_tab_type).unwrap_or(ItemDepotType::Invalid);
        let craft_showing =
            CraftShowingType::try_from(r.showing_type).unwrap_or(CraftShowingType::None);

        let kind = match depot_type {
            ItemDepotType::Weapon => ItemKind::Weapon,
            ItemDepotType::WeaponGem => ItemKind::WeaponGem {
                craft_slot: craft_showing,
            },
            ItemDepotType::Equip => ItemKind::Equip {
                slot: craft_showing,
            },
            ItemDepotType::SpecialItem => ItemKind::SpecialItem,
            ItemDepotType::MissionItem => ItemKind::MissionItem,
            ItemDepotType::Factory => ItemKind::Factory,
            ItemDepotType::Invalid => ItemKind::Unknown {
                raw_tab_type: r.valuable_tab_type,
            },
        };

        ItemConfig {
            id: r.id,
            item_type: r.item_type,
            name: r.name,
            desc: r.desc,
            rarity: r.rarity,
            sort_id1: r.sort_id1,
            sort_id2: r.sort_id2,
            icon_id: r.icon_id,
            model_key: r.model_key,
            max_backpack_stack_count: r.max_backpack_stack_count,
            max_stack_count: r.max_stack_count,
            can_discard: r.backpack_can_discard,
            price: r.price,
            obtain_way_ids: r.obtain_way_ids,
            depot_type,
            kind,
        }
    }
}

pub struct ItemAssets {
    by_id: HashMap<String, ItemConfig>,
    by_depot: HashMap<ItemDepotType, Vec<String>>,
    /// Exp gained per item use, keyed by item id. Sourced from `expItemDataMap`.
    exp_by_id: HashMap<String, i64>,
}

impl ItemAssets {
    pub(crate) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("Item.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;
        let file: ItemFile =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;

        let mut by_id = HashMap::with_capacity(file.item_table.len());
        let mut by_depot: HashMap<ItemDepotType, Vec<String>> = HashMap::new();

        for (_key, raw) in file.item_table {
            let cfg = ItemConfig::from(raw);
            by_depot
                .entry(cfg.depot_type)
                .or_default()
                .push(cfg.id.clone());
            by_id.insert(cfg.id.clone(), cfg);
        }

        for ids in by_depot.values_mut() {
            ids.sort_by(|a, b| {
                let ca = &by_id[a];
                let cb = &by_id[b];
                ca.sort_id1
                    .cmp(&cb.sort_id1)
                    .then(ca.sort_id2.cmp(&cb.sort_id2))
            });
        }

        let exp_by_id = file
            .exp_item_data_map
            .into_iter()
            .map(|(id, data)| (id, data.exp_gain))
            .collect();

        Ok(Self {
            by_id,
            by_depot,
            exp_by_id,
        })
    }

    #[inline]
    pub fn get(&self, id: &str) -> Option<&ItemConfig> {
        self.by_id.get(id)
    }

    #[inline]
    pub fn ids_by_depot(&self, depot: ItemDepotType) -> &[String] {
        self.by_depot.get(&depot).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn iter_by_depot(&self, depot: ItemDepotType) -> impl Iterator<Item = &ItemConfig> {
        self.ids_by_depot(depot)
            .iter()
            .filter_map(|id| self.by_id.get(id))
    }

    #[inline]
    pub fn contains(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    #[inline]
    pub fn count(&self) -> usize {
        self.by_id.len()
    }

    #[inline]
    pub fn count_by_depot(&self, depot: ItemDepotType) -> usize {
        self.by_depot.get(&depot).map(Vec::len).unwrap_or(0)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ItemConfig> {
        self.by_id.values()
    }

    /// Returns the character exp gained from consuming one unit of `item_id`.
    ///
    /// Returns 0 for items that are not character exp cards.
    #[inline]
    pub fn char_exp_for_item(&self, item_id: &str) -> i64 {
        self.exp_by_id.get(item_id).copied().unwrap_or(0)
    }
}
