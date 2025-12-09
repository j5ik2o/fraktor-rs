//! Shared wrapper for actor reference providers registry.

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess},
};

use super::actor_ref_providers::ActorRefProvidersGeneric;

/// Shared wrapper for [`ActorRefProvidersGeneric`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying registry, allowing safe
/// concurrent access from multiple owners.
pub(crate) struct ActorRefProvidersSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<ActorRefProvidersGeneric<TB>, TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type ActorRefProvidersShared = ActorRefProvidersSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ActorRefProvidersSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided actor reference providers registry.
  #[must_use]
  pub(crate) fn new(providers: ActorRefProvidersGeneric<TB>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(providers)) }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for ActorRefProvidersSharedGeneric<TB> {
  fn default() -> Self {
    Self::new(ActorRefProvidersGeneric::default())
  }
}

impl<TB: RuntimeToolbox> Clone for ActorRefProvidersSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<ActorRefProvidersGeneric<TB>> for ActorRefProvidersSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&ActorRefProvidersGeneric<TB>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ActorRefProvidersGeneric<TB>) -> R) -> R {
    self.inner.with_write(f)
  }
}
