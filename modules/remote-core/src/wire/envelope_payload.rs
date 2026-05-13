//! Serialized payload metadata carried by an envelope frame.

#[cfg(test)]
#[path = "envelope_payload_test.rs"]
mod tests;

use alloc::string::String;

use bytes::Bytes;

/// Serialized message payload carried by [`EnvelopePdu`](super::EnvelopePdu).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvelopePayload {
  pub(crate) serializer_id: u32,
  pub(crate) manifest:      Option<String>,
  pub(crate) bytes:         Bytes,
}

impl EnvelopePayload {
  /// Creates a new serialized envelope payload.
  #[must_use]
  pub const fn new(serializer_id: u32, manifest: Option<String>, bytes: Bytes) -> Self {
    Self { serializer_id, manifest, bytes }
  }

  /// Returns the serializer identifier.
  #[must_use]
  pub const fn serializer_id(&self) -> u32 {
    self.serializer_id
  }

  /// Returns the optional serializer manifest.
  #[must_use]
  pub fn manifest(&self) -> Option<&str> {
    self.manifest.as_deref()
  }

  /// Returns the serialized payload bytes.
  #[must_use]
  pub const fn bytes(&self) -> &Bytes {
    &self.bytes
  }
}
