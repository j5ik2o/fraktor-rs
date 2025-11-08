//! Serialization extension identifier.

use super::r#impl::Serialization;
use crate::{RuntimeToolbox, extension::ExtensionId, system::ActorSystemGeneric};

/// Global serialization extension identifier.
pub struct SerializationExtensionId;

/// Singleton extension identifier instance.
pub static SERIALIZATION_EXTENSION: SerializationExtensionId = SerializationExtensionId;

impl<TB: RuntimeToolbox + 'static> ExtensionId<TB> for SerializationExtensionId {
  type Ext = Serialization<TB>;

  fn create_extension(&self, _system: &ActorSystemGeneric<TB>) -> Self::Ext {
    Serialization::new()
  }
}
