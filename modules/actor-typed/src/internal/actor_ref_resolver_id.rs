//! Identifier used to register the typed actor-ref resolver extension.

use fraktor_actor_core_rs::{actor::extension::ExtensionId, system::ActorSystem};

use crate::ActorRefResolver;

/// Identifier for the built-in [`ActorRefResolver`] extension.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ActorRefResolverId;

impl ActorRefResolverId {
  /// Creates a new identifier instance.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self
  }
}

impl ExtensionId for ActorRefResolverId {
  type Ext = ActorRefResolver;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    ActorRefResolver::new(system)
  }
}
