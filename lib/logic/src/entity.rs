use std::collections::HashMap;

use crate::traits::Classified;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityKind {
    Character,
    Enemy,
    Interactive,
    Npc,
    Projectile,
    Creature,
}

// Cleared on every scene transition.
#[derive(Debug, Clone)]
pub struct SceneEntity {
    pub id: u64,
    pub template_id: String,
    pub kind: EntityKind,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    /// The `levelLogicId` from the lv_data file. Used as `origin_id` in
    /// `SceneMonster` so the client knows the AI/behaviour config.
    pub level_logic_id: u64,
    pub belong_level_script_id: i32,
}

#[derive(Debug, Default)]
pub struct EntityManager {
    entities: HashMap<u64, SceneEntity>,
    next_monster_id: u64,
}

impl EntityManager {
    pub fn new() -> Self {
        Self::default()
    }

    // Monster IDs start at 1000 so they don't collide with character IDs (which start at 1).
    pub fn next_monster_id(&mut self) -> u64 {
        let id = 1000 + self.next_monster_id;
        self.next_monster_id += 1;
        id
    }

    /// Read-only view of the next monster id this manager would hand out
    /// (already accounting for the +1000 offset).
    pub fn peek_next_monster_id(&self) -> u64 {
        1000 + self.next_monster_id
    }

    /// Ensure the next monster id is at least `at_least`. No-op if the
    /// counter is already past it. Used by save/migration code.
    pub fn bump_next_monster_id_to(&mut self, at_least: u64) {
        let internal = at_least.saturating_sub(1000);
        if internal > self.next_monster_id {
            self.next_monster_id = internal;
        }
    }

    // Inserting the same id twice is an update, make sure IDs come from
    // `next_monster_id()` or character object IDs to avoid accidental collisions.
    pub fn insert(&mut self, entity: SceneEntity) {
        self.entities.insert(entity.id, entity);
    }

    pub fn remove(&mut self, id: u64) -> Option<SceneEntity> {
        self.entities.remove(&id)
    }

    pub fn get(&self, id: u64) -> Option<&SceneEntity> {
        self.entities.get(&id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut SceneEntity> {
        self.entities.get_mut(&id)
    }

    pub fn contains(&self, id: u64) -> bool {
        self.entities.contains_key(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &SceneEntity> {
        self.entities.values()
    }

    pub fn monsters(&self) -> impl Iterator<Item = &SceneEntity> {
        self.entities.values().filter(|e| e.is_enemy())
    }

    pub fn characters(&self) -> impl Iterator<Item = &SceneEntity> {
        self.entities.values().filter(|e| e.is_character())
    }

    pub fn interactives(&self) -> impl Iterator<Item = &SceneEntity> {
        self.entities.values().filter(|e| e.is_interactive())
    }

    pub fn npcs(&self) -> impl Iterator<Item = &SceneEntity> {
        self.entities.values().filter(|e| e.is_npc())
    }

    // Nukes all entities and resets the ID counter. Call on scene transition.
    pub fn clear(&mut self) {
        self.entities.clear();
        self.next_monster_id = 0;
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn ids(&self) -> Vec<u64> {
        self.entities.keys().copied().collect()
    }

    pub fn ids_by_kind(&self, kind: EntityKind) -> Vec<u64> {
        self.entities
            .values()
            .filter(|e| e.kind == kind)
            .map(|e| e.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entity(id: u64, kind: EntityKind) -> SceneEntity {
        SceneEntity {
            id,
            template_id: format!("tmpl_{}", id),
            kind,
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            level_logic_id: id,
            belong_level_script_id: 0,
        }
    }

    #[test]
    fn new_manager_is_empty() {
        let mgr = EntityManager::new();
        assert_eq!(mgr.len(), 0);
        assert!(mgr.iter().next().is_none());
    }

    #[test]
    fn insert_and_get() {
        let mut mgr = EntityManager::new();
        let e = make_entity(1, EntityKind::Character);
        mgr.insert(e);
        assert!(mgr.contains(1));
        assert!(mgr.get(1).is_some());
        assert_eq!(mgr.get(1).unwrap().template_id, "tmpl_1");
    }

    #[test]
    fn insert_replaces_same_id() {
        let mut mgr = EntityManager::new();
        mgr.insert(make_entity(1, EntityKind::Character));
        mgr.insert(make_entity(1, EntityKind::Enemy));
        assert_eq!(mgr.len(), 1);
        assert_eq!(mgr.get(1).unwrap().kind, EntityKind::Enemy);
    }

    #[test]
    fn remove_entity() {
        let mut mgr = EntityManager::new();
        mgr.insert(make_entity(1, EntityKind::Character));
        let removed = mgr.remove(1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().kind, EntityKind::Character);
        assert!(!mgr.contains(1));
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let mut mgr = EntityManager::new();
        assert!(mgr.remove(999).is_none());
    }

    #[test]
    fn next_monster_id_starts_at_1000() {
        let mut mgr = EntityManager::new();
        assert_eq!(mgr.next_monster_id(), 1000);
        assert_eq!(mgr.next_monster_id(), 1001);
        assert_eq!(mgr.next_monster_id(), 1002);
    }

    #[test]
    fn peek_next_monster_id_does_not_advance() {
        let mut mgr = EntityManager::new();
        assert_eq!(mgr.peek_next_monster_id(), 1000);
        assert_eq!(mgr.peek_next_monster_id(), 1000);
        mgr.next_monster_id();
        assert_eq!(mgr.peek_next_monster_id(), 1001);
    }

    #[test]
    fn bump_next_monster_id() {
        let mut mgr = EntityManager::new();
        mgr.bump_next_monster_id_to(1050);
        assert_eq!(mgr.next_monster_id(), 1050);
    }

    #[test]
    fn bump_next_monster_id_noop_if_lower() {
        let mut mgr = EntityManager::new();
        mgr.next_monster_id(); // now at 1001
        mgr.bump_next_monster_id_to(1000); // below current internal counter
        assert_eq!(mgr.next_monster_id(), 1001);
    }

    #[test]
    fn bump_next_monster_id_saturating_sub() {
        let mut mgr = EntityManager::new();
        // Bumping to 500 (below 1000 offset) should not break anything
        mgr.bump_next_monster_id_to(500);
        assert_eq!(mgr.next_monster_id(), 1000);
    }

    #[test]
    fn clear_resets_everything() {
        let mut mgr = EntityManager::new();
        mgr.insert(make_entity(1, EntityKind::Character));
        mgr.next_monster_id();
        mgr.next_monster_id();
        mgr.clear();
        assert_eq!(mgr.len(), 0);
        assert_eq!(mgr.next_monster_id(), 1000);
    }

    #[test]
    fn ids_returns_all_ids() {
        let mut mgr = EntityManager::new();
        mgr.insert(make_entity(5, EntityKind::Npc));
        mgr.insert(make_entity(10, EntityKind::Enemy));
        let mut ids = mgr.ids();
        ids.sort();
        assert_eq!(ids, vec![5, 10]);
    }

    #[test]
    fn ids_by_kind_filters_correctly() {
        let mut mgr = EntityManager::new();
        mgr.insert(make_entity(1, EntityKind::Character));
        mgr.insert(make_entity(2, EntityKind::Enemy));
        mgr.insert(make_entity(3, EntityKind::Enemy));
        mgr.insert(make_entity(4, EntityKind::Npc));
        let mut enemy_ids = mgr.ids_by_kind(EntityKind::Enemy);
        enemy_ids.sort();
        assert_eq!(enemy_ids, vec![2, 3]);
        assert!(mgr.ids_by_kind(EntityKind::Interactive).is_empty());
    }

    #[test]
    fn filtered_iterators() {
        let mut mgr = EntityManager::new();
        mgr.insert(make_entity(1, EntityKind::Character));
        mgr.insert(make_entity(2, EntityKind::Enemy));
        mgr.insert(make_entity(3, EntityKind::Npc));
        mgr.insert(make_entity(4, EntityKind::Interactive));
        assert_eq!(mgr.monsters().count(), 1);
        assert_eq!(mgr.characters().count(), 1);
        assert_eq!(mgr.npcs().count(), 1);
        assert_eq!(mgr.interactives().count(), 1);
    }

    #[test]
    fn get_mut_returns_mutable_ref() {
        let mut mgr = EntityManager::new();
        mgr.insert(make_entity(1, EntityKind::Character));
        if let Some(e) = mgr.get_mut(1) {
            e.pos_x = 42.0;
        }
        assert_eq!(mgr.get(1).unwrap().pos_x, 42.0);
    }
}
