use anyhow::{Context, Result};
use perlica_logic::bitset::BitsetManager;
use perlica_logic::character::char_bag::CharBag;
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
}

pub struct PlayerDb {
    dir: PathBuf,
}

impl PlayerDb {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create saves dir: {}", dir.display()))?;
        Ok(Self { dir })
    }

    pub async fn load(&self, uid: &str) -> Result<Option<PlayerRecord>> {
        let path = self.path(uid);
        if !path.exists() {
            return Ok(None);
        }
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read save: {}", path.display()))?;
        let mut record: PlayerRecord = bincode::deserialize(&bytes)
            .with_context(|| format!("failed to deserialize save for {uid}"))?;

        // Repair any data inconsistencies that may have crept in (mismatched
        // weapon references, stale cache fields, etc.).
        record.char_bag.validate_after_load();

        Ok(Some(record))
    }

    pub async fn save(
        &self,
        uid: &str,
        char_bag: &CharBag,
        world: &WorldState,
        bitsets: &BitsetManager,
        checkpoint: Option<&CheckpointInfo>,
        revival_mode: RevivalMode,
    ) -> Result<()> {
        let bytes = bincode::serialize(&PlayerRecord {
            char_bag: char_bag.clone(),
            world: world.clone(),
            bitsets: bitsets.clone(),
            checkpoint: checkpoint.cloned(),
            revival_mode,
        })
        .context("failed to serialize save")?;

        let path = self.path(uid);
        let tmp = path.with_extension("bin.tmp");
        fs::write(&tmp, &bytes)
            .with_context(|| format!("failed to write tmp: {}", tmp.display()))?;
        fs::rename(&tmp, &path).with_context(|| format!("failed to rename: {}", path.display()))?;
        Ok(())
    }

    fn path(&self, uid: &str) -> PathBuf {
        self.dir.join(format!("{uid}.bin"))
    }
}
