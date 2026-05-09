//! Name registry collection for actor naming.

use core::marker::PhantomData;

use ahash::RandomState;
use hashbrown::HashMap;

use crate::actor::{Pid, spawn::NameRegistry};

/// Collection of name registries indexed by parent [`Pid`].
pub(crate) struct Registries {
  map:     HashMap<Option<Pid>, NameRegistry, RandomState>,
  _marker: PhantomData<()>,
}

impl Registries {
  /// Creates a new empty registries collection.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()), _marker: PhantomData }
  }

  /// Returns a mutable reference to the registry for the given parent, inserting a new one if
  /// absent.
  pub(crate) fn entry_or_insert(&mut self, parent: Option<Pid>) -> &mut NameRegistry {
    self.map.entry(parent).or_default()
  }

  /// Returns a mutable reference to the registry for the given parent if it exists.
  pub(crate) fn get_mut(&mut self, parent: &Option<Pid>) -> Option<&mut NameRegistry> {
    self.map.get_mut(parent)
  }
}

impl Default for Registries {
  fn default() -> Self {
    Self::new()
  }
}
