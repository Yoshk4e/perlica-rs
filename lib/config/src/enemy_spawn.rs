use crate::tables::enemy_spawn::{EnemySpawnsTable, SceneEnemyInfo};
use std::collections::HashMap;
use anyhow::{Context, Result};
use std::path::Path;

pub struct EnemySpawnAssets {
    data: HashMap<String, Vec<SceneEnemyInfo>>,
}

impl EnemySpawnAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("EnemySpawns.json");
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let table: EnemySpawnsTable = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        Ok(Self {
            data: table.scene,
        })	
    }
	
	pub fn get(&self, scene_id: &str) -> Option<&Vec<SceneEnemyInfo>> {
        self.data.get(scene_id)
    }
	
	//pub fn iter_enemy(&self, scene_id: &str) -> impl Iterator<Item = &Vec<SceneEnemyInfo>> {
    //    self.data.get(scene_id).iter()
    //}
	
	//pub fn get_enemy(&self, scene_id: &str, template_id: &str, position: &Vector3f) -> Option<&SceneEnemyInfo> {
    //    let scene = self.get(scene_id)?;
    //    scene
    //        .iter()
    //        .find
    //        .find(|attr| attr.level == level && attr.break_stage == break_stage)
    //}
	
	pub fn scene_count(&self) -> usize {
        self.data.len()
    }
	
	//pub fn enemy_count(&self, scene_id: &str) -> usize {
    //    let scene: self.data.get(scene_id)?;
	//	scene
    //}
}