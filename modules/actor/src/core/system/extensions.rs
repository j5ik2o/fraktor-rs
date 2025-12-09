//! Extension registry for actor system extensions.

use core::any::{Any, TypeId};

use ahash::RandomState;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};
use hashbrown::HashMap;

/// Registry of actor system extensions.
pub(crate) struct ExtensionsGeneric<TB: RuntimeToolbox + 'static> {
  map:     HashMap<TypeId, ArcShared<dyn Any + Send + Sync + 'static>, RandomState>,
  _marker: core::marker::PhantomData<TB>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type Extensions = ExtensionsGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ExtensionsGeneric<TB> {
  /// Creates a new empty extensions registry.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()), _marker: core::marker::PhantomData }
  }

  /// Returns `true` when an extension for the provided [`TypeId`] is registered.
  #[must_use]
  pub(crate) fn contains_key(&self, type_id: &TypeId) -> bool {
    self.map.contains_key(type_id)
  }

  /// Returns an extension by [`TypeId`].
  pub(crate) fn get(&self, type_id: &TypeId) -> Option<&ArcShared<dyn Any + Send + Sync + 'static>> {
    self.map.get(type_id)
  }

  /// Inserts an extension.
  pub(crate) fn insert(&mut self, type_id: TypeId, extension: ArcShared<dyn Any + Send + Sync + 'static>) {
    self.map.insert(type_id, extension);
  }

  /// Returns an iterator over the extension values.
  pub(crate) fn values(&self) -> impl Iterator<Item = &ArcShared<dyn Any + Send + Sync + 'static>> {
    self.map.values()
  }
}

impl<TB: RuntimeToolbox + 'static> Default for ExtensionsGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
