use std::collections::HashMap;

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

impl SceneEntity {
    pub fn position(&self) -> (f32, f32, f32) {
        (self.pos_x, self.pos_y, self.pos_z)
    }
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
        self.entities
            .values()
            .filter(|e| e.kind == EntityKind::Enemy)
    }

    pub fn characters(&self) -> impl Iterator<Item = &SceneEntity> {
        self.entities
            .values()
            .filter(|e| e.kind == EntityKind::Character)
    }

    // Nukes all entities and resets the ID counter. Call on scene transition.
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
