//! Shared wrapper for serialization extension instance.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::extension::SerializationExtensionGeneric;
use crate::core::extension::Extension;

/// Shared wrapper for a serialization extension instance.
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying extension, allowing safe
/// concurrent access from multiple owners.
pub struct SerializationExtensionSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<SerializationExtensionGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
pub type SerializationExtensionShared = SerializationExtensionSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> SerializationExtensionSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided extension instance.
  #[must_use]
  pub fn new(extension: SerializationExtensionGeneric<TB>) -> Self {
    let mutex = <TB::MutexFamily as SyncMutexFamily>::create(extension);
    Self { inner: ArcShared::new(mutex) }
  }
}

impl<TB: RuntimeToolbox> Clone for SerializationExtensionSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<SerializationExtensionGeneric<TB>>
  for SerializationExtensionSharedGeneric<TB>
{
  fn with_read<R>(&self, f: impl FnOnce(&SerializationExtensionGeneric<TB>) -> R) -> R {
    let guard = self.inner.lock();
    f(&*guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut SerializationExtensionGeneric<TB>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut *guard)
  }
}

impl<TB: RuntimeToolbox + 'static> Extension<TB> for SerializationExtensionSharedGeneric<TB> {}
