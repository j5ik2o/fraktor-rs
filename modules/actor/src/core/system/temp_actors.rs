//! Temporary actor registry for `/temp` path actors.

use alloc::string::String;

use ahash::RandomState;
use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};
use hashbrown::HashMap;

use crate::core::actor_prim::actor_ref::ActorRefGeneric;

/// Registry of temporary actor references.
pub(crate) struct TempActorsGeneric<TB: RuntimeToolbox + 'static> {
  map: HashMap<String, ActorRefGeneric<TB>, RandomState>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type TempActors = TempActorsGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> TempActorsGeneric<TB> {
  /// Creates a new empty temporary actors registry.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()) }
  }

  /// Inserts an actor reference under the given name.
  pub(crate) fn insert(&mut self, name: String, actor: ActorRefGeneric<TB>) {
    self.map.insert(name, actor);
  }

  /// Removes and returns an actor reference if present.
  pub(crate) fn remove(&mut self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.map.remove(name)
  }

  /// Returns a registered actor reference if present.
  #[must_use]
  pub(crate) fn get(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.map.get(name).cloned()
  }
}

impl<TB: RuntimeToolbox + 'static> Default for TempActorsGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
