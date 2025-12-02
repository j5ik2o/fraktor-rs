//! Shared wrapper for ActorRefProvider implementations.

use core::any::TypeId;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::ActorRefProvider;
use crate::core::{
  actor_prim::{
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRefGeneric,
  },
  error::ActorError,
};

/// Shared wrapper that provides thread-safe access to an [`ActorRefProvider`]
/// implementation.
///
/// This adapter wraps a provider in a `ToolboxMutex`, allowing it to be shared
/// across multiple owners while satisfying the `&mut self` requirement of
/// `ActorRefProvider` methods.
///
/// # Usage
///
/// 1. Create a shared wrapper: `ActorRefProviderShared::new(provider)`
/// 2. Clone and share as needed
/// 3. Call provider methods through the wrapper (automatically acquires lock)
pub struct ActorRefProviderSharedGeneric<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + 'static> {
  inner:   ArcShared<ToolboxMutex<P, TB>>,
  schemes: &'static [ActorPathScheme],
}

impl<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + 'static> ActorRefProviderSharedGeneric<TB, P> {
  /// Creates a new shared wrapper around the provided implementation.
  pub fn new(provider: P) -> Self {
    let schemes = provider.supported_schemes();
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(provider)), schemes }
  }

  /// Creates a new shared wrapper from an existing shared mutex.
  #[must_use]
  pub const fn from_shared(inner: ArcShared<ToolboxMutex<P, TB>>, schemes: &'static [ActorPathScheme]) -> Self {
    Self { inner, schemes }
  }

  /// Returns a reference to the inner shared mutex.
  #[must_use]
  pub const fn inner(&self) -> &ArcShared<ToolboxMutex<P, TB>> {
    &self.inner
  }

  /// Returns the type ID of the inner provider type.
  #[must_use]
  pub fn inner_type_id(&self) -> TypeId {
    TypeId::of::<P>()
  }

  /// Creates an actor reference for the provided path.
  ///
  /// This method uses `&self` instead of `&mut self` because the internal mutex
  /// provides the necessary synchronization. This allows the shared wrapper to be
  /// used in contexts that require `Fn` closures.
  ///
  /// # Errors
  ///
  /// Returns an error if the actor reference cannot be created.
  pub fn get_actor_ref(&self, path: ActorPath) -> Result<ActorRefGeneric<TB>, ActorError> {
    self.inner.lock().actor_ref(path)
  }
}

impl<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + 'static> Clone for ActorRefProviderSharedGeneric<TB, P> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), schemes: self.schemes }
  }
}

impl<TB: RuntimeToolbox + 'static, P: ActorRefProvider<TB> + 'static> ActorRefProvider<TB>
  for ActorRefProviderSharedGeneric<TB, P>
{
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    self.schemes
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRefGeneric<TB>, ActorError> {
    self.inner.lock().actor_ref(path)
  }
}

/// Type alias for [`ActorRefProviderSharedGeneric`] using the default [`NoStdToolbox`].
pub type ActorRefProviderShared<P> = ActorRefProviderSharedGeneric<NoStdToolbox, P>;
