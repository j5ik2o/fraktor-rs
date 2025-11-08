use alloc::string::String;

use serde::{Deserialize, Serialize};

use super::bytes::Bytes;

/// Serialized payload transmitted between nodes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SerializedPayload {
  serializer_id: u32,
  manifest:      String,
  bytes:         Bytes,
}

impl SerializedPayload {
  /// Creates a new payload from its pieces.
  #[must_use]
  pub const fn new(serializer_id: u32, manifest: String, bytes: Bytes) -> Self {
    Self { serializer_id, manifest, bytes }
  }

  /// Returns the serializer identifier.
  #[must_use]
  pub const fn serializer_id(&self) -> u32 {
    self.serializer_id
  }

  /// Returns the manifest string.
  #[must_use]
  pub fn manifest(&self) -> &str {
    &self.manifest
  }

  /// Returns the raw bytes.
  #[must_use]
  pub const fn bytes(&self) -> &Bytes {
    &self.bytes
  }

  /// Consumes the payload and returns its byte buffer.
  #[must_use]
  pub fn into_bytes(self) -> Bytes {
    self.bytes
  }
}
