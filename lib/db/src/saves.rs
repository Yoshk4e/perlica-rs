use crate::error::{DbError, Result};
use perlica_logic::bitset::BitsetManager;
use perlica_logic::character::char_bag::CharBag;
use perlica_logic::mail::MailManager;
use perlica_logic::mission::{GuideManager, MissionManager};
use perlica_logic::player::WorldState;
use perlica_logic::scene::{CheckpointInfo, RevivalMode};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// # What is saved and why
///
/// - `char_bag`: All characters, teams, skill levels, and the embedded
///   `WeaponDepot`. This is the primary player progression record.
///   Weapons live inside `CharBag.weapon_depot`; there is no separate
///   top-level weapon field.
///
/// - `world`: Current scene name, player position/rotation, role level and
///   exp. Position is synced from `MovementManager` into `WorldState` just
///   before this record is written so the player respawns at their last
///   known location.
///
/// - `bitsets`: All boolean flag sets (items found, areas visited, wiki
///   entries read, etc.).
///
/// - `checkpoint`: The last repatriate / rest-point the player activated.
///   `None` for new players. Needed so the revival flow sends the player
///   back to the right spot after a full wipe.
///
/// - `revival_mode`: Whether the player is using the default respawn, a
///   repatriate point, or a dungeon checkpoint. Persisted so the correct
///   mode is restored on login.
///
#[derive(Serialize, Deserialize)]
pub struct PlayerRecord {
    pub char_bag: CharBag,
    pub world: WorldState,
    #[serde(default)]
    pub bitsets: BitsetManager,
    #[serde(default)]
    pub checkpoint: Option<CheckpointInfo>,
    #[serde(default)]
    pub revival_mode: RevivalMode,
    #[serde(default)]
    pub missions: MissionManager,
    #[serde(default)]
    pub guides: GuideManager,
    #[serde(default)]
    pub mail: MailManager,
}

/// A strictly borrowed version of `PlayerRecord` used to avoid allocations
/// and cloning during the serialization process.
/// According to DotRh, suggestion of cloning everywhere being a bad idea.
#[derive(Serialize)]
pub struct PlayerRecordRef<'a> {
    pub char_bag: &'a CharBag,
    pub world: &'a WorldState,
    pub bitsets: &'a BitsetManager,
    pub checkpoint: Option<&'a CheckpointInfo>,
    pub revival_mode: RevivalMode,
    pub missions: &'a MissionManager,
    pub guides: &'a GuideManager,
    pub mail: &'a MailManager,
}

impl<'a> PlayerRecordRef<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        char_bag: &'a CharBag,
        world: &'a WorldState,
        bitsets: &'a BitsetManager,
        checkpoint: Option<&'a CheckpointInfo>,
        revival_mode: RevivalMode,
        missions: &'a MissionManager,
        guides: &'a GuideManager,
        mail: &'a MailManager,
    ) -> Self {
        Self {
            char_bag,
            world,
            bitsets,
            checkpoint,
            revival_mode,
            missions,
            guides,
            mail,
        }
    }
}

pub struct PlayerDb {
    dir: PathBuf,
}

impl PlayerDb {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir).map_err(|e| DbError::CreateDir {
            path: dir.clone(),
            source: e,
        })?;
        Ok(Self { dir })
    }

    pub async fn load(&self, uid: &str) -> Result<Option<PlayerRecord>> {
        let path = self.path(uid);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path).map_err(|e| DbError::ReadSave {
            path: path.clone(),
            source: e,
        })?;
        let mut record: PlayerRecord =
            bincode::deserialize(&bytes).map_err(|e| DbError::Deserialize {
                uid: uid.to_string(),
                source: e,
            })?;

        // Repair any data inconsistencies that may have crept in (mismatched
        // weapon references, stale cache fields, etc.).
        record.char_bag.validate_after_load();

        Ok(Some(record))
    }

    pub async fn save<'a>(&self, uid: &str, record_ref: PlayerRecordRef<'a>) -> Result<()> {
        let bytes = bincode::serialize(&record_ref)?;

        let path = self.path(uid);
        let tmp = path.with_extension("bin.tmp");
        fs::write(&tmp, &bytes).map_err(|e| DbError::WriteTmp {
            path: tmp.clone(),
            source: e,
        })?;
        fs::rename(&tmp, &path).map_err(|e| DbError::Rename {
            path: path.clone(),
            source: e,
        })?;

        Ok(())
    }

    fn path(&self, uid: &str) -> PathBuf {
        self.dir.join(format!("{uid}.bin"))
    }
}
