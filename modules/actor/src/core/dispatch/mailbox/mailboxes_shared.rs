//! Shared wrapper for mailbox registry.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::MailboxesGeneric;

/// Shared wrapper for [`MailboxesGeneric`] that hides the lock behind closure-based APIs.
pub struct MailboxesSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<MailboxesGeneric<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for MailboxesSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for MailboxesSharedGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox + 'static> MailboxesSharedGeneric<TB> {
  /// Creates a new shared mailbox registry.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(MailboxesGeneric::new())) }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<MailboxesGeneric<TB>> for MailboxesSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&MailboxesGeneric<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut MailboxesGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

/// Type alias for [`MailboxesSharedGeneric`] using the default [`NoStdToolbox`].
pub type MailboxesShared = MailboxesSharedGeneric<NoStdToolbox>;
