//! Shared wrapper for actor factory.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::factory::ActorFactory;

/// Shared wrapper for [`ActorFactory`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying factory, allowing safe
/// concurrent access from multiple owners.
pub struct ActorFactorySharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn ActorFactory<TB>>, TB>>,
}

/// Type alias using the default toolbox.
pub type ActorFactoryShared = ActorFactorySharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ActorFactorySharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided actor factory.
  #[must_use]
  pub fn new(factory: Box<dyn ActorFactory<TB>>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(factory)) }
  }
}

impl<TB: RuntimeToolbox> Clone for ActorFactorySharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn ActorFactory<TB>>> for ActorFactorySharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ActorFactory<TB>>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ActorFactory<TB>>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
