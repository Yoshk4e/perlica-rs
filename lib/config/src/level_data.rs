//! Level data loader.
//!
//! Loads all `*lv_data*.json` files from `assets/level_data/`, merging
//! multiple sub-files for the same scene by `sceneId`. Exposes the full
//! scene contents — enemies, interactives, NPCs, level scripts, patrols,
//! enemy groups, factory regions, splines, and safe zones.
//!
//! Expected asset layout:
//! ```text
//! assets/
//!   level_data/
//!     map01_lv001_lv_data.json
//!     map01_lv001_lv_data_sub01.json
//!     map01_lv002_lv_data.json
//!     ...
//! ```

use crate::error::{ConfigError, Result};
use crate::tables::level_data::{
    LvDataFile, LvEnemy, LvEnemyGroup, LvFactoryRegion, LvInteractive, LvLevelScript, LvNpc,
    LvPatrol, LvSpline,
};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, warn};

/// Merged scene data for one scene, assembled from all its sub-files.
#[derive(Debug, Default)]
pub struct SceneData {
    pub enemies: Vec<LvEnemy>,
    pub enemy_groups: Vec<LvEnemyGroup>,
    pub patrols: Vec<LvPatrol>,
    pub interactives: Vec<LvInteractive>,
    pub npcs: Vec<LvNpc>,
    pub level_scripts: Vec<LvLevelScript>,
    pub factory_regions: Vec<LvFactoryRegion>,
    pub splines: Vec<LvSpline>,
}

pub struct LevelDataAssets {
    scenes: HashMap<String, SceneData>,
}

impl LevelDataAssets {
    pub(super) fn load(config_dir: &Path) -> Result<Self> {
        let level_data_dir = config_dir.join("level_data");
        let mut scenes: HashMap<String, SceneData> = HashMap::new();

        if !level_data_dir.exists() {
            warn!(
                "level_data directory not found at '{}', no scene data loaded",
                level_data_dir.display()
            );
            return Ok(Self { scenes });
        }

        let mut file_count = 0u32;

        for entry in std::fs::read_dir(&level_data_dir).map_err(|e| ConfigError::ReadDir {
            path: level_data_dir.clone(),
            source: e,
        })? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .contains("lv_data")
            {
                continue;
            }

            let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
                path: path.clone(),
                source: e,
            })?;

            let file: LvDataFile = match serde_json::from_str(&contents) {
                Ok(f) => f,
                Err(e) => {
                    warn!(
                        "Skipping malformed lv_data file '{}': {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };

            let scene = scenes.entry(file.scene_id.clone()).or_default();

            // Enemies — skip defaultHide (revealed by level scripts)
            scene
                .enemies
                .extend(file.enemies.into_iter().filter(|e| !e.base.default_hide));
            scene.enemy_groups.extend(file.enemy_groups);
            scene.patrols.extend(file.patrols);

            // Interactives — keep all; defaultHide ones are managed by level scripts
            scene
                .npcs
                .extend(file.npcs.into_iter().filter(|n| !n.base.default_hide));
            scene.interactives.extend(
                file.interactives
                    .into_iter()
                    .filter(|i| !i.base.default_hide),
            );
            scene.level_scripts.extend(file.level_scripts);
            scene.factory_regions.extend(file.factory_regions);
            scene.splines.extend(file.splines);

            file_count += 1;
        }

        debug!(
            "Loaded {} lv_data files covering {} scenes",
            file_count,
            scenes.len()
        );

        Ok(Self { scenes })
    }

    /// Full merged scene data. `None` if the scene has no lv_data files.
    pub fn get(&self, scene_id: &str) -> Option<&SceneData> {
        self.scenes.get(scene_id)
    }

    /// Convenience: enemies for a scene (skips defaultHide, already filtered on load).
    pub fn enemies(&self, scene_id: &str) -> &[LvEnemy] {
        self.scenes
            .get(scene_id)
            .map(|s| s.enemies.as_slice())
            .unwrap_or_default()
    }

    /// Convenience: interactives for a scene.
    pub fn interactives(&self, scene_id: &str) -> &[LvInteractive] {
        self.scenes
            .get(scene_id)
            .map(|s| s.interactives.as_slice())
            .unwrap_or_default()
    }

    /// Convenience: level scripts for a scene.
    pub fn level_scripts(&self, scene_id: &str) -> &[LvLevelScript] {
        self.scenes
            .get(scene_id)
            .map(|s| s.level_scripts.as_slice())
            .unwrap_or_default()
    }

    /// Convenience: NPCs for a scene.
    pub fn npcs(&self, scene_id: &str) -> &[LvNpc] {
        self.scenes
            .get(scene_id)
            .map(|s| s.npcs.as_slice())
            .unwrap_or_default()
    }

    /// Look up a patrol by id within a scene.
    pub fn patrol(&self, scene_id: &str, patrol_id: u64) -> Option<&LvPatrol> {
        self.scenes
            .get(scene_id)?
            .patrols
            .iter()
            .find(|p| p.id == patrol_id)
    }

    /// Look up an enemy group by id within a scene.
    pub fn enemy_group(
        &self,
        scene_id: &str,
        group_id: u64,
    ) -> Option<&crate::tables::level_data::LvEnemyGroup> {
        self.scenes
            .get(scene_id)?
            .enemy_groups
            .iter()
            .find(|g| g.group_id == group_id)
    }

    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }
}
