//! Actor cell registry.

use ahash::RandomState;
use fraktor_utils_core_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use crate::core::kernel::actor::{ActorCell, Pid};

/// Registry of actor cells indexed by their [`Pid`].
pub(crate) struct Cells {
  map: HashMap<Pid, ArcShared<ActorCell>, RandomState>,
}
#[allow(dead_code)]
impl Cells {
  /// Creates a new empty cell registry.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()) }
  }

  /// Inserts an actor cell into the registry.
  pub(crate) fn insert(&mut self, pid: Pid, cell: ArcShared<ActorCell>) {
    self.map.insert(pid, cell);
  }

  /// Removes an actor cell from the registry.
  #[must_use]
  pub(crate) fn remove(&mut self, pid: &Pid) -> Option<ArcShared<ActorCell>> {
    self.map.remove(pid)
  }

  /// Returns an actor cell by its [`Pid`].
  #[must_use]
  pub(crate) fn get(&self, pid: &Pid) -> Option<ArcShared<ActorCell>> {
    self.map.get(pid).cloned()
  }
}

impl Default for Cells {
  fn default() -> Self {
    Self::new()
  }
}
