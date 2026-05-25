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
    #[inline]
    fn is_empty(&self) -> bool {
        WeaponDepot::is_empty(self)
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
    #[inline]
    fn is_empty(&self) -> bool {
        GemDepot::is_empty(self)
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
impl DepotKind for GemDepot {
    const DEPOT_TYPE: i32 = GemDepot::DEPOT_TYPE;
}

impl Container for EquipDepot {
    type Item = EquipInstance;
    #[inline]
    fn len(&self) -> usize {
        EquipDepot::len(self)
    }
    #[inline]
    fn is_empty(&self) -> bool {
        EquipDepot::is_empty(self)
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
    #[inline]
    fn is_empty(&self) -> bool {
        StackableDepot::is_empty(self)
    }
}

impl Container for EntityManager {
    type Item = SceneEntity;
    #[inline]
    fn len(&self) -> usize {
        EntityManager::len(self)
    }
    #[inline]
    fn is_empty(&self) -> bool {
        EntityManager::is_empty(self)
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
    #[inline]
    fn is_empty(&self) -> bool {
        self.mails.is_empty()
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
