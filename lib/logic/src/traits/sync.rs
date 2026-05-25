//! Transient -> persistent state synchronisation.

use crate::movement::MovementManager;
use crate::player::WorldState;

/// `Self` knows how to push its mutable runtime state into a persistent
/// holder of type `T`.
///
/// The canonical example is `MovementManager -> WorldState`, the manager
/// is the live, mutated copy; the world state is the serialized snapshot.
/// Every "transient runtime vs persisted snapshot" pair in the codebase
/// fits this pattern.
pub trait SyncWriteBack<T> {
    fn write_back_into(&self, target: &mut T);
}

// retardism but well for now
impl SyncWriteBack<WorldState> for MovementManager {
    #[inline]
    fn write_back_into(&self, target: &mut WorldState) {
        self.sync_to_world(target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writeback_movement_to_world() {
        let mut world = WorldState::default();
        let mut mvmt = MovementManager::from(&world);
        mvmt.update_position(1.0, 2.0, 3.0);
        mvmt.write_back_into(&mut world);
        assert_eq!((world.pos_x, world.pos_y, world.pos_z), (1.0, 2.0, 3.0));
    }
}
