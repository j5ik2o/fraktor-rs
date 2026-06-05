//! Bridge value for cluster payload kind and actor-core serialized metadata.

#[cfg(test)]
#[path = "cluster_serialized_message_test.rs"]
mod tests;

use fraktor_actor_core_kernel_rs::serialization::{SerializedMessage, SerializerId};

use super::ClusterMessagePayloadKind;

/// Immutable bridge value for cluster payload kind and actor-core serialized metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSerializedMessage {
  payload_kind:       ClusterMessagePayloadKind,
  serialized_message: SerializedMessage,
}

impl ClusterSerializedMessage {
  /// Creates a new cluster serialized message.
  #[must_use]
  pub const fn new(payload_kind: ClusterMessagePayloadKind, serialized_message: SerializedMessage) -> Self {
    Self { payload_kind, serialized_message }
  }

  /// Returns the cluster payload kind.
  #[must_use]
  pub const fn payload_kind(&self) -> ClusterMessagePayloadKind {
    self.payload_kind
  }

  /// Returns the actor-core serialized message.
  #[must_use]
  pub const fn serialized_message(&self) -> &SerializedMessage {
    &self.serialized_message
  }

  /// Returns the actor-core serializer identifier.
  #[must_use]
  pub const fn serializer_id(&self) -> SerializerId {
    self.serialized_message.serializer_id()
  }

  /// Returns the actor-core manifest.
  #[must_use]
  pub fn manifest(&self) -> Option<&str> {
    self.serialized_message.manifest()
  }

  /// Returns the actor-core serialized payload bytes.
  #[must_use]
  pub fn payload_bytes(&self) -> &[u8] {
    self.serialized_message.bytes()
  }
}
