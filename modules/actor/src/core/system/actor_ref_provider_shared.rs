//! Shared wrapper for ActorRefProvider implementations.

use core::any::TypeId;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::{ActorRefProvider, ActorRefProviderHandle};
use crate::core::{
  actor::{
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRefGeneric,
  },
  error::ActorError,
};

/// Shared wrapper that provides thread-safe access to an [`ActorRefProvider`]
/// implementation.
///
/// This adapter wraps a provider handle in a `ToolboxMutex`, allowing it to be shared
/// across multiple owners while satisfying the `&mut self` requirement of
/// `ActorRefProvider` methods. The wrapper itself remains a thin layer that
/// locks and delegates only.
///
/// # Usage
///
/// 1. Create a shared wrapper: `ActorRefProviderShared::new(provider)`
/// 2. Clone and share as needed
/// 3. Call provider methods through the wrapper (automatically acquires lock)
pub struct ActorRefProviderSharedGeneric<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + 'static> {
  inner: ArcShared<ToolboxMutex<ActorRefProviderHandle<P>, TB>>,
}

impl<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + 'static> ActorRefProviderSharedGeneric<TB, P> {
  /// Creates a new shared wrapper around the provided implementation.
  pub fn new(provider: P) -> Self {
    let schemes = provider.supported_schemes();
    let handle = ActorRefProviderHandle::new(provider, schemes);
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(handle)) }
  }

  /// Creates a new shared wrapper from an existing shared mutex.
  #[must_use]
  pub const fn from_shared(inner: ArcShared<ToolboxMutex<ActorRefProviderHandle<P>, TB>>) -> Self {
    Self { inner }
  }

  /// Returns a reference to the inner shared mutex.
  #[must_use]
  pub const fn inner(&self) -> &ArcShared<ToolboxMutex<ActorRefProviderHandle<P>, TB>> {
    &self.inner
  }

  /// Returns the type ID of the inner provider type.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)]
  pub fn inner_type_id(&self) -> TypeId {
    TypeId::of::<P>()
  }

  /// Creates an actor reference for the provided path.
  ///
  /// Thin wrapper: lock, delegate, unlock.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor reference cannot be created.
  pub fn get_actor_ref(&self, path: ActorPath) -> Result<ActorRefGeneric<TB>, ActorError> {
    let mut guard = self.inner.lock();
    guard.actor_ref(path)
  }
}

impl<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + 'static> Clone for ActorRefProviderSharedGeneric<TB, P> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + 'static> ActorRefProvider<TB>
  for ActorRefProviderSharedGeneric<TB, P>
{
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    let guard = self.inner.lock();
    guard.supported_schemes()
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRefGeneric<TB>, ActorError> {
    let mut guard = self.inner.lock();
    guard.actor_ref(path)
  }
}

impl<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + 'static> SharedAccess<ActorRefProviderHandle<P>>
  for ActorRefProviderSharedGeneric<TB, P>
{
  fn with_read<R>(&self, f: impl FnOnce(&ActorRefProviderHandle<P>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ActorRefProviderHandle<P>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}

/// Type alias for [`ActorRefProviderSharedGeneric`] using the default [`NoStdToolbox`].
pub type ActorRefProviderShared<P> = ActorRefProviderSharedGeneric<NoStdToolbox, P>;
