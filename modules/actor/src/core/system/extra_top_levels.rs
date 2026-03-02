//! Extra top-level actor registry for custom top-level paths.

use ahash::RandomState;
use hashbrown::HashMap;

use crate::core::actor::actor_ref::ActorRef;

/// Registry of extra top-level actor references.
pub(crate) struct ExtraTopLevels {
  map: HashMap<alloc::string::String, ActorRef, RandomState>,
}
#[allow(dead_code)]
impl ExtraTopLevels {
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
  pub(crate) fn insert(&mut self, name: alloc::string::String, actor: ActorRef) {
    self.map.insert(name, actor);
  }

  /// Returns a registered actor reference if present.
  #[must_use]
  pub(crate) fn get(&self, name: &str) -> Option<ActorRef> {
    self.map.get(name).cloned()
  }
}

impl Default for ExtraTopLevels {
  fn default() -> Self {
    Self::new()
  }
}
