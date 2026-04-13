//! Shared wrapper for ActorRefProvider implementations.

use alloc::string::String;
use core::{any::TypeId, marker::PhantomData};

use fraktor_utils_core_rs::core::sync::{SharedAccess, SharedLock, DefaultMutex};

use super::{ActorRefProvider, ActorRefProviderHandle};
use crate::core::kernel::{
  actor::{
    Address,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRef,
    deploy::Deployer,
    error::ActorError,
  },
  system::TerminationSignal,
};

/// Shared wrapper that provides thread-safe access to an [`ActorRefProvider`]
/// implementation.
///
/// This adapter wraps a provider handle in a `SharedLock`, allowing it to be shared
/// across multiple owners while satisfying the `&mut self` requirement of
/// `ActorRefProvider` methods. The wrapper itself remains a thin layer that
/// locks and delegates only.
///
/// # Usage
///
/// 1. Materialize a shared handle via `ActorRefProviderHandleShared::new`
/// 2. Clone and share as needed
/// 3. Call provider methods through the wrapper (automatically acquires lock)
pub struct ActorRefProviderHandleShared<P: ActorRefProvider + 'static> {
  inner:   SharedLock<ActorRefProviderHandle<P>>,
  _marker: PhantomData<()>,
}

impl<P: ActorRefProvider + 'static> ActorRefProviderHandleShared<P> {
  /// Creates a new shared wrapper using the builtin spin lock backend.
  #[must_use]
  pub fn new(provider: P) -> Self {
    let schemes = provider.supported_schemes();
    Self::from_shared_lock(SharedLock::new_with_driver::<DefaultMutex<_>>(ActorRefProviderHandle::new(
      provider, schemes,
    )))
  }

  /// Creates a new shared wrapper from an existing shared lock.
  #[must_use]
  pub const fn from_shared_lock(inner: SharedLock<ActorRefProviderHandle<P>>) -> Self {
    Self { inner, _marker: PhantomData }
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
    self.inner.with_lock(|guard| guard.actor_ref(path))
  }

  /// Resolves an actor reference through the shared wrapper without requiring an outer mutable
  /// borrow.
  ///
  /// # Errors
  ///
  /// Returns an error if the provider cannot resolve the path.
  pub fn resolve_actor_ref(&self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.inner.with_lock(|guard| guard.resolve_actor_ref(path))
  }

  /// Resolves an actor reference from its canonical string form through the shared wrapper.
  ///
  /// # Errors
  ///
  /// Returns an error if the string is not a valid actor path or resolution fails.
  pub fn resolve_actor_ref_str(&self, path: &str) -> Result<ActorRef, ActorError> {
    self.inner.with_lock(|guard| guard.resolve_actor_ref_str(path))
  }
}

impl<P: ActorRefProvider + 'static> Clone for ActorRefProviderHandleShared<P> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
  }
}

impl<P: ActorRefProvider + 'static> ActorRefProvider for ActorRefProviderHandleShared<P> {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.inner.with_read(|guard| guard.supported_schemes())
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.inner.with_lock(|guard| guard.actor_ref(path))
  }

  fn root_guardian(&self) -> Option<ActorRef> {
    self.inner.with_read(|guard| guard.root_guardian())
  }

  fn guardian(&self) -> Option<ActorRef> {
    self.inner.with_read(|guard| guard.guardian())
  }

  fn system_guardian(&self) -> Option<ActorRef> {
    self.inner.with_read(|guard| guard.system_guardian())
  }

  fn dead_letters(&self) -> ActorRef {
    self.inner.with_read(|guard| guard.dead_letters())
  }

  fn temp_path(&self) -> ActorPath {
    self.inner.with_read(|guard| guard.temp_path())
  }

  fn root_path(&self) -> ActorPath {
    self.inner.with_read(|guard| guard.root_path())
  }

  fn root_guardian_at(&self, address: &Address) -> Option<ActorRef> {
    self.inner.with_read(|guard| guard.root_guardian_at(address))
  }

  fn deployer(&self) -> Option<Deployer> {
    self.inner.with_read(|guard| guard.deployer())
  }

  fn resolve_actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    self.inner.with_lock(|guard| guard.resolve_actor_ref(path))
  }

  fn resolve_actor_ref_str(&mut self, path: &str) -> Result<ActorRef, ActorError> {
    self.inner.with_lock(|guard| guard.resolve_actor_ref_str(path))
  }

  fn temp_path_with_prefix(&self, prefix: &str) -> Result<ActorPath, ActorError> {
    self.inner.with_read(|guard| guard.temp_path_with_prefix(prefix))
  }

  fn temp_container(&self) -> Option<ActorRef> {
    self.inner.with_read(|guard| guard.temp_container())
  }

  fn register_temp_actor(&self, actor: ActorRef) -> Option<String> {
    self.inner.with_lock(|guard| guard.register_temp_actor(actor))
  }

  fn unregister_temp_actor(&self, name: &str) {
    self.inner.with_lock(|guard| guard.unregister_temp_actor(name));
  }

  fn unregister_temp_actor_path(&self, path: &ActorPath) -> Result<(), ActorError> {
    self.inner.with_lock(|guard| guard.unregister_temp_actor_path(path))
  }

  fn temp_actor(&self, name: &str) -> Option<ActorRef> {
    self.inner.with_read(|guard| guard.temp_actor(name))
  }

  fn termination_signal(&self) -> TerminationSignal {
    self.inner.with_read(|guard| guard.termination_signal())
  }

  fn get_external_address_for(&self, addr: &Address) -> Option<Address> {
    self.inner.with_read(|guard| guard.get_external_address_for(addr))
  }

  fn get_default_address(&self) -> Option<Address> {
    self.inner.with_read(|guard| guard.get_default_address())
  }
}

impl<P: ActorRefProvider + 'static> SharedAccess<ActorRefProviderHandle<P>> for ActorRefProviderHandleShared<P> {
  fn with_read<R>(&self, f: impl FnOnce(&ActorRefProviderHandle<P>) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut ActorRefProviderHandle<P>) -> R) -> R {
    self.inner.with_write(f)
  }
}
