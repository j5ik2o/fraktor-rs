//! Shared wrapper for unbounded priority message queue state.

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, SpinSyncMutex};

use super::unbounded_priority_message_queue_state::UnboundedPriorityMessageQueueState;

/// Shared wrapper around unbounded-priority mailbox state.
pub struct UnboundedPriorityMessageQueueStateShared {
  inner: SharedLock<UnboundedPriorityMessageQueueState>,
}

impl UnboundedPriorityMessageQueueStateShared {
  /// Creates a new shared wrapper using the builtin spin lock backend.
  #[must_use]
  pub fn new(state: UnboundedPriorityMessageQueueState) -> Self {
    Self::from_shared_lock(SharedLock::new_with_driver::<SpinSyncMutex<_>>(state))
  }

  /// Creates a shared wrapper from an existing shared lock.
  #[must_use]
  pub const fn from_shared_lock(inner: SharedLock<UnboundedPriorityMessageQueueState>) -> Self {
    Self { inner }
  }
}

impl Clone for UnboundedPriorityMessageQueueStateShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<UnboundedPriorityMessageQueueState> for UnboundedPriorityMessageQueueStateShared {
  fn with_read<R>(&self, f: impl FnOnce(&UnboundedPriorityMessageQueueState) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut UnboundedPriorityMessageQueueState) -> R) -> R {
    self.inner.with_write(f)
  }
}
