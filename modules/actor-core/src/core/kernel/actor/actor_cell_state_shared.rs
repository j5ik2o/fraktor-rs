//! Shared wrapper for actor-cell runtime state.

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock};

use crate::core::kernel::actor::{ActorCellState, ActorLockFactory};

/// Stable shared wrapper for actor-cell runtime state.
#[derive(Clone)]
pub struct ActorCellStateShared {
  inner: SharedLock<ActorCellState>,
}

impl ActorCellStateShared {
  /// Creates actor-cell runtime state with the requested lock driver family.
  #[must_use]
  pub fn new_with_lock_factory(factory: &impl ActorLockFactory) -> Self {
    Self { inner: factory.create_lock(ActorCellState::new()) }
  }

  pub(crate) fn with_read<R>(&self, f: impl FnOnce(&ActorCellState) -> R) -> R {
    self.inner.with_read(f)
  }

  pub(crate) fn with_write<R>(&self, f: impl FnOnce(&mut ActorCellState) -> R) -> R {
    self.inner.with_write(f)
  }
}
