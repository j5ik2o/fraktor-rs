//! Extra top-level actor registry for custom top-level paths.

use ahash::RandomState;
use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};
use hashbrown::HashMap;

use crate::core::actor::actor_ref::ActorRefGeneric;

/// Registry of extra top-level actor references.
pub(crate) struct ExtraTopLevelsGeneric<TB: RuntimeToolbox + 'static> {
  map: HashMap<alloc::string::String, ActorRefGeneric<TB>, RandomState>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type ExtraTopLevels = ExtraTopLevelsGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ExtraTopLevelsGeneric<TB> {
  /// Creates a new empty extra top-levels registry.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()) }
  }

  /// Checks if a name is already registered.
  #[must_use]
  pub(crate) fn contains_key(&self, name: &str) -> bool {
    self.map.contains_key(name)
  }

  /// Inserts an actor reference under the given name.
  pub(crate) fn insert(&mut self, name: alloc::string::String, actor: ActorRefGeneric<TB>) {
    self.map.insert(name, actor);
  }

  /// Returns a registered actor reference if present.
  #[must_use]
  pub(crate) fn get(&self, name: &str) -> Option<ActorRefGeneric<TB>> {
    self.map.get(name).cloned()
  }
}

impl<TB: RuntimeToolbox + 'static> Default for ExtraTopLevelsGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
