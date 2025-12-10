//! Actor reference provider callers registry by scheme.

use alloc::boxed::Box;

use ahash::RandomState;
use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox};
use hashbrown::HashMap;

use crate::core::{
  actor_prim::{actor_path::ActorPathScheme, actor_ref::ActorRefGeneric},
  error::ActorError,
};

/// Type alias for an actor reference provider caller function.
pub(crate) type ActorRefProviderCaller<TB> = Box<
  dyn Fn(crate::core::actor_prim::actor_path::ActorPath) -> Result<ActorRefGeneric<TB>, ActorError>
    + Send
    + Sync
    + 'static,
>;

/// Registry of actor reference provider callers by scheme.
pub(crate) struct ActorRefProviderCallersGeneric<TB: RuntimeToolbox + 'static> {
  map: HashMap<ActorPathScheme, ActorRefProviderCaller<TB>, RandomState>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type ActorRefProviderCallers = ActorRefProviderCallersGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ActorRefProviderCallersGeneric<TB> {
  /// Creates a new empty actor reference provider callers registry.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()) }
  }

  /// Returns a caller for the provided scheme.
  pub(crate) fn get(&self, scheme: ActorPathScheme) -> Option<&ActorRefProviderCaller<TB>> {
    self.map.get(&scheme)
  }

  /// Inserts a caller for the provided scheme.
  pub(crate) fn insert(&mut self, scheme: ActorPathScheme, caller: ActorRefProviderCaller<TB>) {
    self.map.insert(scheme, caller);
  }
}

impl<TB: RuntimeToolbox + 'static> Default for ActorRefProviderCallersGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
