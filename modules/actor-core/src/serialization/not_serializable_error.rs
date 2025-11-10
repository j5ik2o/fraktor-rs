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

  /// Returns the serializer identifier that caused the failure (if known).
  #[must_use]
  pub const fn serializer_id(&self) -> Option<SerializerId> {
    self.serializer_id
  }

  /// Returns the manifest string that triggered the failure (if present).
  #[must_use]
  pub fn manifest(&self) -> Option<&str> {
    self.manifest.as_deref()
  }

  /// Returns the transport diagnostic hint (if any).
  #[must_use]
  pub fn transport_hint(&self) -> Option<&TransportInformation> {
    self.transport_hint.as_ref()
  }
}
