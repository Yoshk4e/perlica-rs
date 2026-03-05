use crate::net::NetContext;
use perlica_proto::{BitsetData, CsBitsetRemove, ScBitsetRemove, ScSyncAllBitset};
use tracing::{debug, instrument, warn};

// Beyond.GEnums.BitsetType
#[derive(Debug, Clone, Copy, PartialEq)]
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

#[instrument(skip(ctx), fields(uid = %ctx.player.uid))]
pub async fn on_cs_bitset_remove(ctx: &mut NetContext<'_>, req: CsBitsetRemove) -> ScBitsetRemove {
    let name = match BitsetType::from_i32(req.r#type) {
        Some(t) => format!("{:?}", t),
        None => "Unknown".to_string(),
    };

    debug!(
        bitset_type = %name,
        type_id = req.r#type,
        bits = ?req.value,
        "remove bit"
    );

    ScBitsetRemove {
        r#type: req.r#type,
        value: req.value.clone(),
        source: 0, // 0 = success / normal
    }
}

pub async fn push_bitsets(ctx: &mut NetContext<'_>) -> bool {
    let bitset = (1..20)
        .map(|t| BitsetData {
            r#type: t,
            value: vec![],
        })
        .collect();

    ctx.notify(ScSyncAllBitset { bitset }).await.is_ok()
}
