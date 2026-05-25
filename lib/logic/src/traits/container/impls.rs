//! Concrete [`Container`] / [`KeyedContainer`] / [`IdAllocator`] /
//! [`DepotKind`] impls for every depot and manager.
//!
//! Pure plumbing, no behaviour. Kept private to the parent module so
//! downstream code sees the *capabilities* on the public types without ever
//! having to import this file.

use super::{Container, DepotKind, IdAllocator, KeyedContainer};
use crate::entity::{EntityManager, SceneEntity};
use crate::item::{
    EquipDepot, EquipInstId, EquipInstance, GemDepot, GemInstId, GemInstance, StackableDepot,
    WeaponDepot, WeaponInstId, WeaponInstance,
};
use crate::mail::{MailManager, StoredMail};

impl Container for WeaponDepot {
    type Item = WeaponInstance;
    #[inline]
    fn len(&self) -> usize {
        WeaponDepot::len(self)
    }
}
impl KeyedContainer for WeaponDepot {
    type Key = WeaponInstId;
    #[inline]
    fn contains_key(&self, key: Self::Key) -> bool {
        self.contains(key)
    }
    #[inline]
    fn get_ref(&self, key: Self::Key) -> Option<&WeaponInstance> {
        self.get(key)
    }
    #[inline]
    fn get_mut_ref(&mut self, key: Self::Key) -> Option<&mut WeaponInstance> {
        self.get_mut(key)
    }
}
impl IdAllocator for WeaponDepot {
    type Id = WeaponInstId;
    #[inline]
    fn peek_next_id(&self) -> u64 {
        self.next_inst_id()
    }
    #[inline]
    fn bump_next_id_to(&mut self, at_least: u64) {
        if at_least > self.next_inst_id() {
            self.set_next_inst_id(at_least);
        }
    }
}
impl DepotKind for WeaponDepot {
    const DEPOT_TYPE: i32 = WeaponDepot::DEPOT_TYPE;
}

impl Container for GemDepot {
    type Item = GemInstance;
    #[inline]
    fn len(&self) -> usize {
        GemDepot::len(self)
    }
}
impl KeyedContainer for GemDepot {
    type Key = GemInstId;
    #[inline]
    fn contains_key(&self, key: Self::Key) -> bool {
        self.contains(key)
    }
    #[inline]
    fn get_ref(&self, key: Self::Key) -> Option<&GemInstance> {
        self.get(key)
    }
    #[inline]
    fn get_mut_ref(&mut self, key: Self::Key) -> Option<&mut GemInstance> {
        self.get_mut(key)
    }
}
impl IdAllocator for GemDepot {
    type Id = GemInstId;
    #[inline]
    fn peek_next_id(&self) -> u64 {
        self.next_inst_id()
    }
    #[inline]
    fn bump_next_id_to(&mut self, at_least: u64) {
        if at_least > self.next_inst_id() {
            self.set_next_inst_id(at_least);
        }
    }
}
impl DepotKind for GemDepot {
    const DEPOT_TYPE: i32 = GemDepot::DEPOT_TYPE;
}

impl Container for EquipDepot {
    type Item = EquipInstance;
    #[inline]
    fn len(&self) -> usize {
        EquipDepot::len(self)
    }
}
impl KeyedContainer for EquipDepot {
    type Key = EquipInstId;
    #[inline]
    fn contains_key(&self, key: Self::Key) -> bool {
        self.contains(key)
    }
    #[inline]
    fn get_ref(&self, key: Self::Key) -> Option<&EquipInstance> {
        self.get(key)
    }
    #[inline]
    fn get_mut_ref(&mut self, key: Self::Key) -> Option<&mut EquipInstance> {
        self.get_mut(key)
    }
}
impl IdAllocator for EquipDepot {
    type Id = EquipInstId;
    #[inline]
    fn peek_next_id(&self) -> u64 {
        self.next_inst_id()
    }
    #[inline]
    fn bump_next_id_to(&mut self, at_least: u64) {
        if at_least > self.next_inst_id() {
            self.set_next_inst_id(at_least);
        }
    }
}
impl DepotKind for EquipDepot {
    const DEPOT_TYPE: i32 = EquipDepot::DEPOT_TYPE;
}

impl Container for StackableDepot {
    /// Pseudo-item, stackables are addressed by `&str` template id, but the
    /// trait wants something concrete, so we report the count of distinct
    /// template ids.
    type Item = u32;
    #[inline]
    fn len(&self) -> usize {
        StackableDepot::len(self)
    }
}

impl Container for EntityManager {
    type Item = SceneEntity;
    #[inline]
    fn len(&self) -> usize {
        EntityManager::len(self)
    }
}
impl KeyedContainer for EntityManager {
    type Key = u64;
    #[inline]
    fn contains_key(&self, key: u64) -> bool {
        self.contains(key)
    }
    #[inline]
    fn get_ref(&self, key: u64) -> Option<&SceneEntity> {
        self.get(key)
    }
    #[inline]
    fn get_mut_ref(&mut self, key: u64) -> Option<&mut SceneEntity> {
        self.get_mut(key)
    }
}

impl Container for MailManager {
    type Item = StoredMail;
    #[inline]
    fn len(&self) -> usize {
        self.mails.len()
    }
}
impl KeyedContainer for MailManager {
    type Key = u64;
    #[inline]
    fn contains_key(&self, key: u64) -> bool {
        self.mails.iter().any(|m| m.mail_id == key)
    }
    #[inline]
    fn get_ref(&self, key: u64) -> Option<&StoredMail> {
        self.mails.iter().find(|m| m.mail_id == key)
    }
    #[inline]
    fn get_mut_ref(&mut self, key: u64) -> Option<&mut StoredMail> {
        self.mails.iter_mut().find(|m| m.mail_id == key)
    }
}
impl IdAllocator for MailManager {
    /// Mail ids are bare `u64` (no newtype), so the generic `IdAllocator`
    /// surface still works thanks to the `InstanceId for u64` blanket impl
    /// in [`crate::traits::id`].
    type Id = u64;
    #[inline]
    fn peek_next_id(&self) -> u64 {
        self.next_id()
    }
    #[inline]
    fn bump_next_id_to(&mut self, at_least: u64) {
        if at_least > self.next_id() {
            self.set_next_id(at_least);
        }
    }
}

// EntityManager allocates monster ids via an internal counter.  Exposing it
// as `IdAllocator` lets the same save / migration helpers reseed the
// counter without naming the concrete type.
impl IdAllocator for EntityManager {
    type Id = u64;
    #[inline]
    fn peek_next_id(&self) -> u64 {
        // EntityManager offsets ids by 1000 internally; expose the next id
        // it *would* hand out so migration code can keep pace.
        self.peek_next_monster_id()
    }
    #[inline]
    fn bump_next_id_to(&mut self, at_least: u64) {
        self.bump_next_monster_id_to(at_least);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::IdAllocator;

    #[test]
    fn gem_depot_id_allocator() {
        let mut d = GemDepot::new();
        let start = d.peek_next_id();
        d.bump_next_id_to(start + 10);
        assert!(d.peek_next_id() >= start + 10);
        // Lower bumps are no-ops.
        d.bump_next_id_to(0);
        assert!(d.peek_next_id() >= start + 10);
    }

    #[test]
    fn equip_depot_id_allocator() {
        let mut d = EquipDepot::new();
        let start = d.peek_next_id();
        d.bump_next_id_to(start + 5);
        assert!(d.peek_next_id() >= start + 5);
    }

    #[test]
    fn mail_manager_id_allocator() {
        let mut m = MailManager::new();
        let start = m.peek_next_id();
        m.bump_next_id_to(start + 3);
        assert!(m.peek_next_id() >= start + 3);
    }
}
