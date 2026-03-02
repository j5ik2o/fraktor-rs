//! Shared wrapper for ActorRefProvider implementations.

use core::{any::TypeId, marker::PhantomData};

use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeMutex,
  sync::{ArcShared, SharedAccess},
};

use super::{ActorRefProvider, ActorRefProviderHandle};
use crate::core::{
  actor::{
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRef,
  },
  error::ActorError,
};

/// Shared wrapper that provides thread-safe access to an [`ActorRefProvider`]
/// implementation.
///
/// This adapter wraps a provider handle in a `RuntimeMutex`, allowing it to be shared
/// across multiple owners while satisfying the `&mut self` requirement of
/// `ActorRefProvider` methods. The wrapper itself remains a thin layer that
/// locks and delegates only.
///
/// # Usage
///
/// 1. Create a shared wrapper: `ActorRefProviderShared::new(provider)`
/// 2. Clone and share as needed
/// 3. Call provider methods through the wrapper (automatically acquires lock)
pub struct ActorRefProviderShared<P: ActorRefProvider + 'static> {
  inner:   ArcShared<RuntimeMutex<ActorRefProviderHandle<P>>>,
  _marker: PhantomData<()>,
}

impl<P: ActorRefProvider + 'static> ActorRefProviderShared<P> {
  /// Creates a new shared wrapper around the provided implementation.
  pub fn new(provider: P) -> Self {
    let schemes = provider.supported_schemes();
    let handle = ActorRefProviderHandle::new(provider, schemes);
    Self { inner: ArcShared::new(RuntimeMutex::new(handle)), _marker: PhantomData }
  }

  /// Creates a new shared wrapper from an existing shared mutex.
  #[must_use]
  pub const fn from_shared(inner: ArcShared<RuntimeMutex<ActorRefProviderHandle<P>>>) -> Self {
    Self { inner, _marker: PhantomData }
  }

  /// Returns a reference to the inner shared mutex.
  #[must_use]
  pub const fn inner(&self) -> &ArcShared<RuntimeMutex<ActorRefProviderHandle<P>>> {
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
  pub fn get_actor_ref(&self, path: ActorPath) -> Result<ActorRef, ActorError> {
    let mut guard = self.inner.lock();
    guard.actor_ref(path)
  }
}

impl<P: ActorRefProvider + 'static> Clone for ActorRefProviderShared<P> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
  }
}

impl<P: ActorRefProvider + 'static> ActorRefProvider for ActorRefProviderShared<P> {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    let guard = self.inner.lock();
    guard.supported_schemes()
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    let mut guard = self.inner.lock();
    guard.actor_ref(path)
  }
}

impl<P: ActorRefProvider + 'static> SharedAccess<ActorRefProviderHandle<P>> for ActorRefProviderShared<P> {
  fn with_read<R>(&self, f: impl FnOnce(&ActorRefProviderHandle<P>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ActorRefProviderHandle<P>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
