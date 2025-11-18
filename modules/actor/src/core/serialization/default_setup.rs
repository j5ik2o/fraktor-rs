//! Helpers for provisioning the default serialization extension.

use alloc::vec::Vec;

use hashbrown::HashMap;

use super::{builtin, serialization_extension_id::SerializationExtensionId, serialization_setup::SerializationSetup};

/// Returns the default serialization setup used by the runtime.
#[must_use]
pub fn default_serialization_setup() -> SerializationSetup {
  SerializationSetup::from_parts(
    HashMap::new(),
    HashMap::new(),
    HashMap::new(),
    HashMap::new(),
    HashMap::new(),
    Vec::new(),
    builtin::STRING_ID,
    Vec::new(),
  )
}

/// Returns an extension identifier bound to the default setup.
#[must_use]
pub fn default_serialization_extension_id() -> SerializationExtensionId {
  SerializationExtensionId::new(default_serialization_setup())
}
