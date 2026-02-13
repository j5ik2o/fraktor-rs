//! Actor reference provider registry.

use core::any::{Any, TypeId};

use ahash::RandomState;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};
use hashbrown::HashMap;

/// Registry of actor reference providers by type.
pub(crate) struct ActorRefProvidersGeneric<TB: RuntimeToolbox + 'static> {
  map:     HashMap<TypeId, ArcShared<dyn Any + Send + Sync + 'static>, RandomState>,
  _marker: core::marker::PhantomData<TB>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type ActorRefProviders = ActorRefProvidersGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ActorRefProvidersGeneric<TB> {
  /// Creates a new empty actor reference providers registry.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()), _marker: core::marker::PhantomData }
  }

  /// Returns `true` when a provider for the provided [`TypeId`] is registered.
  #[allow(dead_code)]
  #[must_use]
  pub(crate) fn contains_key(&self, type_id: &TypeId) -> bool {
    self.map.contains_key(type_id)
  }

  /// Returns a provider by [`TypeId`].
  pub(crate) fn get(&self, type_id: &TypeId) -> Option<&ArcShared<dyn Any + Send + Sync + 'static>> {
    self.map.get(type_id)
  }

  /// Inserts a provider.
  pub(crate) fn insert(&mut self, type_id: TypeId, provider: ArcShared<dyn Any + Send + Sync + 'static>) {
    self.map.insert(type_id, provider);
  }

  /// Returns an iterator over the provider values.
  #[allow(dead_code)]
  pub(crate) fn values(&self) -> impl Iterator<Item = &ArcShared<dyn Any + Send + Sync + 'static>> {
    self.map.values()
  }
}

impl<TB: RuntimeToolbox + 'static> Default for ActorRefProvidersGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
