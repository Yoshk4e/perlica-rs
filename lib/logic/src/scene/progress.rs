use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub struct SceneProgress {
    destroyed_interactives: HashMap<String, HashSet<u64>>,
    dirty: bool,
}

impl SceneProgress {
    pub fn destroy_interactive(&mut self, scene: &str, id: u64) {
        self.dirty |= self
            .destroyed_interactives
            .entry(scene.to_owned())
            .or_default()
            .insert(id);
    }

    pub fn is_destroyed(&self, scene: &str, id: u64) -> bool {
        self.destroyed_interactives
            .get(scene)
            .is_some_and(|ids| ids.contains(&id))
    }

    pub fn destroyed_interactives(&self) -> impl Iterator<Item = (&str, u64)> {
        self.destroyed_interactives
            .iter()
            .flat_map(|(scene, ids)| ids.iter().map(move |&id| (scene.as_str(), id)))
    }

    pub fn has_pending_changes(&self) -> bool {
        self.dirty
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }
}
