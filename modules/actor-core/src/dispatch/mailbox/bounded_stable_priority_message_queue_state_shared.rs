//! Shared wrapper for bounded stable-priority message queue state.

use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::bounded_stable_priority_message_queue_state::BoundedStablePriorityMessageQueueState;

/// Shared wrapper around bounded stable-priority mailbox state.
pub struct BoundedStablePriorityMessageQueueStateShared {
  inner: SharedLock<BoundedStablePriorityMessageQueueState>,
}

impl BoundedStablePriorityMessageQueueStateShared {
  /// Creates a new shared wrapper using the builtin spin lock backend.
  #[must_use]
  pub fn new(state: BoundedStablePriorityMessageQueueState) -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<DefaultMutex<_>>(state))
  }

  /// Creates a shared wrapper from an existing shared lock.
  #[must_use]
  pub const fn from_shared_lock(inner: SharedLock<BoundedStablePriorityMessageQueueState>) -> Self {
    Self { inner }
  }
}

impl Clone for BoundedStablePriorityMessageQueueStateShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<BoundedStablePriorityMessageQueueState> for BoundedStablePriorityMessageQueueStateShared {
  fn with_read<R>(&self, f: impl FnOnce(&BoundedStablePriorityMessageQueueState) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut BoundedStablePriorityMessageQueueState) -> R) -> R {
    self.inner.with_write(f)
  }
}
