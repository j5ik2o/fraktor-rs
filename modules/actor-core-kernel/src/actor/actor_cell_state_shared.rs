//! Shared wrapper for actor-cell runtime state.

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use crate::actor::ActorCellState;

/// Stable shared wrapper for actor-cell runtime state.
#[derive(Clone)]
pub(crate) struct ActorCellStateShared {
  inner: SharedLock<ActorCellState>,
}

impl ActorCellStateShared {
  /// Creates a new shared wrapper using the builtin spin lock backend.
  #[must_use]
  pub(crate) fn new(state: ActorCellState) -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<DefaultMutex<_>>(state))
  }

  /// Creates a shared wrapper from an existing shared lock.
  #[must_use]
  pub(crate) const fn from_shared_lock(inner: SharedLock<ActorCellState>) -> Self {
    Self { inner }
  }

  pub(crate) fn with_read<R>(&self, f: impl FnOnce(&ActorCellState) -> R) -> R {
    self.inner.with_read(f)
  }

  pub(crate) fn with_write<R>(&self, f: impl FnOnce(&mut ActorCellState) -> R) -> R {
    self.inner.with_write(f)
  }
}
