//! Extension identifier for the serialization subsystem.

use crate::{
  ExtensionId, RuntimeToolbox,
  serialization::{extension::SerializationExtensionGeneric, serialization_setup::SerializationSetup},
  system::ActorSystemGeneric,
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

impl<TB: RuntimeToolbox + 'static> ExtensionId<TB> for SerializationExtensionId {
  type Ext = SerializationExtensionGeneric<TB>;

  fn create_extension(&self, system: &ActorSystemGeneric<TB>) -> Self::Ext {
    SerializationExtensionGeneric::new(system, self.setup.clone())
  }
}
