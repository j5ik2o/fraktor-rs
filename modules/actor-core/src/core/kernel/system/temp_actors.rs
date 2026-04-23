//! Temporary actor registry for `/temp` path actors.

use alloc::string::String;

use ahash::RandomState;
use hashbrown::HashMap;

use crate::core::kernel::actor::{Pid, actor_ref::ActorRef};

/// Registry of temporary actor references.
pub(crate) struct TempActors {
  map: HashMap<String, ActorRef, RandomState>,
}

impl TempActors {
  /// Creates a new empty temporary actors registry.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()) }
  }

  /// Inserts an actor reference under the given name.
  pub(crate) fn insert(&mut self, name: String, actor: ActorRef) {
    self.map.insert(name, actor);
  }

  /// Removes and returns an actor reference if present.
  pub(crate) fn remove(&mut self, name: &str) -> Option<ActorRef> {
    self.map.remove(name)
  }

  /// Returns a registered actor reference if present.
  #[must_use]
  pub(crate) fn get(&self, name: &str) -> Option<ActorRef> {
    self.map.get(name).cloned()
  }

  /// Removes an actor reference by pid if present.
  pub(crate) fn remove_by_pid(&mut self, pid: &Pid) -> Option<(String, ActorRef)> {
    let name = self.map.iter().find(|(_, actor)| actor.pid() == *pid).map(|(name, _)| name.clone());
    name.and_then(|name| self.map.remove(&name).map(|actor| (name, actor)))
  }
}

impl Default for TempActors {
  fn default() -> Self {
    Self::new()
  }
}
