//! Local-only actor reference provider.

use crate::core::kernel::actor::{
  actor_path::{ActorPath, ActorPathScheme},
  actor_ref::ActorRef,
  actor_ref_provider::ActorRefProvider,
  error::ActorError,
};

/// Provider for local-only actor systems.
///
/// This provider only supports local actor references and will return an error
/// if asked to create a reference for a remote actor path (with authority).
pub struct LocalActorRefProvider {
  _marker: core::marker::PhantomData<()>,
}

impl LocalActorRefProvider {
  /// Creates a new local actor reference provider.
  #[must_use]
  pub const fn new() -> Self {
    Self { _marker: core::marker::PhantomData }
  }
}

impl Default for LocalActorRefProvider {
  fn default() -> Self {
    Self::new()
  }
}

impl ActorRefProvider for LocalActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    &[ActorPathScheme::Fraktor]
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    // Local provider only supports local paths (no authority)
    if path.parts().authority_endpoint().is_some() {
      return Err(ActorError::fatal("LocalActorRefProvider does not support remote actor paths"));
    }

    // For local-only systems, actor references are typically created through
    // ActorContext::spawn_child() rather than through the provider.
    // This method is primarily for path-based lookups, which are not yet implemented.
    Err(ActorError::fatal("Path-based actor lookup not yet implemented for local provider"))
  }
}
