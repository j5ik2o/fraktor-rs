//! Shared wrapper for bounded priority message queue state.

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, SpinSyncMutex};

use super::bounded_priority_message_queue_state::BoundedPriorityMessageQueueState;

/// Shared wrapper around bounded-priority mailbox state.
pub struct BoundedPriorityMessageQueueStateShared {
  inner: SharedLock<BoundedPriorityMessageQueueState>,
}

impl BoundedPriorityMessageQueueStateShared {
  /// Creates a new shared wrapper using the builtin spin lock backend.
  #[must_use]
  pub fn new(state: BoundedPriorityMessageQueueState) -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<SpinSyncMutex<_>>(state))
  }

  /// Creates a shared wrapper from an existing shared lock.
  #[must_use]
  pub const fn from_shared_lock(inner: SharedLock<BoundedPriorityMessageQueueState>) -> Self {
    Self { inner }
  }
}

impl Clone for BoundedPriorityMessageQueueStateShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<BoundedPriorityMessageQueueState> for BoundedPriorityMessageQueueStateShared {
  fn with_read<R>(&self, f: impl FnOnce(&BoundedPriorityMessageQueueState) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut BoundedPriorityMessageQueueState) -> R) -> R {
    self.inner.with_write(f)
  }
}
