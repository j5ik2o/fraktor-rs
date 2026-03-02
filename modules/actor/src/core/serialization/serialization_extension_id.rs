//! Extension identifier for the serialization subsystem.

use crate::core::{
  extension::ExtensionId,
  serialization::{
    extension::SerializationExtension, extension_shared::SerializationExtensionShared,
    serialization_setup::SerializationSetup,
  },
  system::ActorSystem,
};

/// Identifier used to register the serialization extension.
#[derive(Clone)]
pub struct SerializationExtensionId {
  setup: SerializationSetup,
}

impl SerializationExtensionId {
  /// Creates a new identifier for the provided setup.
  #[must_use]
  pub const fn new(setup: SerializationSetup) -> Self {
    Self { setup }
  }
}

impl ExtensionId for SerializationExtensionId {
  type Ext = SerializationExtensionShared;

  fn create_extension(&self, system: &ActorSystem) -> Self::Ext {
    let inner = SerializationExtension::new(system, self.setup.clone());
    SerializationExtensionShared::new(inner)
  }
}
