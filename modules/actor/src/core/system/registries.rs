//! Name registry collection for actor naming.

use ahash::RandomState;
use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};
use hashbrown::HashMap;

use crate::core::{actor_prim::Pid, spawn::NameRegistry};

/// Collection of name registries indexed by parent [`Pid`].
pub(crate) struct RegistriesGeneric<TB: RuntimeToolbox + 'static> {
  map:     HashMap<Option<Pid>, NameRegistry, RandomState>,
  _marker: core::marker::PhantomData<TB>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type Registries = RegistriesGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> RegistriesGeneric<TB> {
  /// Creates a new empty registries collection.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()), _marker: core::marker::PhantomData }
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

impl<TB: RuntimeToolbox + 'static> Default for RegistriesGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
