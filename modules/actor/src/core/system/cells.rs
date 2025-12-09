//! Actor cell registry.

use ahash::RandomState;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};
use hashbrown::HashMap;

use crate::core::actor_prim::{ActorCellGeneric, Pid};

/// Registry of actor cells indexed by their [`Pid`].
pub(crate) struct CellsGeneric<TB: RuntimeToolbox + 'static> {
  map: HashMap<Pid, ArcShared<ActorCellGeneric<TB>>, RandomState>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type Cells = CellsGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> CellsGeneric<TB> {
  /// Creates a new empty cell registry.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()) }
  }

  /// Inserts an actor cell into the registry.
  pub(crate) fn insert(&mut self, pid: Pid, cell: ArcShared<ActorCellGeneric<TB>>) {
    self.map.insert(pid, cell);
  }

  /// Removes an actor cell from the registry.
  #[must_use]
  pub(crate) fn remove(&mut self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.map.remove(pid)
  }

  /// Returns an actor cell by its [`Pid`].
  #[must_use]
  pub(crate) fn get(&self, pid: &Pid) -> Option<ArcShared<ActorCellGeneric<TB>>> {
    self.map.get(pid).cloned()
  }

  /// Returns `true` if the registry contains the given [`Pid`].
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn contains(&self, pid: &Pid) -> bool {
    self.map.contains_key(pid)
  }

  /// Returns the number of cells in the registry.
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn len(&self) -> usize {
    self.map.len()
  }

  /// Returns `true` if the registry is empty.
  #[must_use]
  #[allow(dead_code)]
  pub(crate) fn is_empty(&self) -> bool {
    self.map.is_empty()
  }
}

impl<TB: RuntimeToolbox + 'static> Default for CellsGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
