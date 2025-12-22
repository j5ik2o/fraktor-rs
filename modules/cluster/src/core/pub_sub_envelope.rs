//! Serialized pub/sub envelope.

use alloc::{string::String, vec::Vec};

/// Serialized payload envelope used by pub/sub delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubSubEnvelope {
  /// Serializer identifier.
  pub serializer_id: u32,
  /// Type name or manifest.
  pub type_name:     String,
  /// Serialized bytes.
  pub bytes:         Vec<u8>,
}
