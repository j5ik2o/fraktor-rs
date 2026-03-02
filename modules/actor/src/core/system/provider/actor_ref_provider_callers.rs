//! Actor reference provider callers registry by scheme.

use ahash::RandomState;
use fraktor_utils_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use crate::core::{
  actor::{actor_path::ActorPathScheme, actor_ref::ActorRef},
  error::ActorError,
};
pub(crate) type ActorRefProviderCaller =
  ArcShared<dyn Fn(crate::core::actor::actor_path::ActorPath) -> Result<ActorRef, ActorError> + Send + Sync + 'static>;

/// Registry of actor reference provider callers by scheme.
pub(crate) struct ActorRefProviderCallers {
  map: HashMap<ActorPathScheme, ActorRefProviderCaller, RandomState>,
}
#[allow(dead_code)]
impl ActorRefProviderCallers {
  /// Creates a new empty actor reference provider callers registry.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self { map: HashMap::with_hasher(RandomState::new()) }
  }

  /// Returns a caller for the provided scheme.
  pub(crate) fn get(&self, scheme: ActorPathScheme) -> Option<&ActorRefProviderCaller> {
    self.map.get(&scheme)
  }

  /// Inserts a caller for the provided scheme.
  pub(crate) fn insert(&mut self, scheme: ActorPathScheme, caller: ActorRefProviderCaller) {
    self.map.insert(scheme, caller);
  }
}

impl Default for ActorRefProviderCallers {
  fn default() -> Self {
    Self::new()
  }
}
