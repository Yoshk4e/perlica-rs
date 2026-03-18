use std::collections::HashMap;

// What kind of entity is this?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityKind {
    Character,
    Enemy,
    Interactive,
    Npc,
    Projectile,
    Creature,
}

// A single entity currently alive in the scene.
// cleared on every scene transition.
#[derive(Debug, Clone)]
pub struct SceneEntity {
    pub id: u64,
    pub template_id: String,
    pub kind: EntityKind,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
}

impl SceneEntity {
    pub fn position(&self) -> (f32, f32, f32) {
        (self.pos_x, self.pos_y, self.pos_z)
    }
}

// Tracks all entities currently alive in the scene.
#[derive(Debug, Default)]
pub struct EntityManager {
    entities: HashMap<u64, SceneEntity>,
    next_monster_id: u64,
}

impl EntityManager {
    pub fn new() -> Self {
        Self::default()
    }

    // Generate the next unique monster entity ID.
    // IDs start at 1000 so they don't collide with character IDs (which start at 1).
    pub fn next_monster_id(&mut self) -> u64 {
        let id = 1000 + self.next_monster_id;
        self.next_monster_id += 1;
        id
    }

    // Insert (or overwrite) an entity. If you insert the same id twice you just
    // updated it, so make sure IDs come from next_monster_id() or char object IDs for anyone who might use this method in the future. YOLO
    pub fn insert(&mut self, entity: SceneEntity) {
        self.entities.insert(entity.id, entity);
    }

    // Remove by id. returns the entity if it was tracked, None otherwise
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
        self.entities
            .values()
            .filter(|e| e.kind == EntityKind::Enemy)
    }

    pub fn characters(&self) -> impl Iterator<Item = &SceneEntity> {
        self.entities
            .values()
            .filter(|e| e.kind == EntityKind::Character)
    }

    // Nuke all entities and reset the ID counter. Call on scene transition.
    pub fn clear(&mut self) {
        self.entities.clear();
        self.next_monster_id = 0;
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    // Collect all live entity IDs useful for building despawn packet lists
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
