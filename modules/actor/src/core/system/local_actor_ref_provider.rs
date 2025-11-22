//! Local-only actor reference provider.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  actor_prim::{
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRefGeneric,
  },
  error::ActorError,
  system::ActorRefProvider,
};

/// Provider for local-only actor systems.
///
/// This provider only supports local actor references and will return an error
/// if asked to create a reference for a remote actor path (with authority).
pub struct LocalActorRefProviderGeneric<TB: RuntimeToolbox + 'static> {
  _marker: core::marker::PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> LocalActorRefProviderGeneric<TB> {
  /// Creates a new local actor reference provider.
  #[must_use]
  pub const fn new() -> Self {
    Self { _marker: core::marker::PhantomData }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for LocalActorRefProviderGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefProvider<TB> for LocalActorRefProviderGeneric<TB> {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    &[ActorPathScheme::Fraktor]
  }

  fn actor_ref(&self, path: ActorPath) -> Result<ActorRefGeneric<TB>, ActorError> {
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
