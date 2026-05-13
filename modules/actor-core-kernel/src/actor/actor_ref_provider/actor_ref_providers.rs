//! Actor reference provider registry.

use core::{
  any::{Any, TypeId},
  marker::PhantomData,
};

use ahash::RandomState;
use fraktor_utils_core_rs::sync::ArcShared;
use hashbrown::HashMap;

/// Registry of actor reference providers by type.
pub(crate) struct ActorRefProviders {
  map:     HashMap<TypeId, ArcShared<dyn Any + Send + Sync + 'static>, RandomState>,
  _marker: PhantomData<()>,
}
impl ActorRefProviders {
  /// Creates a new empty actor reference providers registry.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()), _marker: PhantomData }
  }

  /// Returns a provider by [`TypeId`].
  pub(crate) fn get(&self, type_id: &TypeId) -> Option<&ArcShared<dyn Any + Send + Sync + 'static>> {
    self.map.get(type_id)
  }

  /// Inserts a provider.
  pub(crate) fn insert(&mut self, type_id: TypeId, provider: ArcShared<dyn Any + Send + Sync + 'static>) {
    self.map.insert(type_id, provider);
  }
}

impl Default for ActorRefProviders {
  fn default() -> Self {
    Self::new()
  }
}
