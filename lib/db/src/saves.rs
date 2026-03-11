use anyhow::{Context, Result};
use perlica_logic::character::char_bag::CharBag;
use perlica_logic::player::WorldState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub struct PlayerRecord {
    pub char_bag: CharBag,
    pub world: WorldState,
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
        let record = bincode::deserialize(&bytes)
            .with_context(|| format!("failed to deserialize save for {uid}"))?;
        Ok(Some(record))
    }

    pub async fn save(&self, uid: &str, char_bag: &CharBag, world: &WorldState) -> Result<()> {
        let bytes = bincode::serialize(&PlayerRecord {
            char_bag: char_bag.clone(),
            world: world.clone(),
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
