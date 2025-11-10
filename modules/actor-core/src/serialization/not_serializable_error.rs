//! Detailed not-serializable error payload.

use alloc::string::String;

use super::{serializer_id::SerializerId, transport_information::TransportInformation};

/// Carries context for serialization failures surfaced to the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotSerializableError {
  type_name:      String,
  serializer_id:  Option<SerializerId>,
  manifest:       Option<String>,
  transport_hint: Option<TransportInformation>,
}

impl NotSerializableError {
  /// Creates a new error payload with the supplied fields.
  #[must_use]
  pub fn new(
    type_name: impl Into<String>,
    serializer_id: Option<SerializerId>,
    manifest: Option<String>,
    transport_hint: Option<TransportInformation>,
  ) -> Self {
    Self {
      type_name: type_name.into(),
      serializer_id,
      manifest,
      transport_hint,
    }
  }

  /// Returns the failing type name.
  #[must_use]
  pub fn type_name(&self) -> &str {
    &self.type_name
  }
}
