//! Versioned cluster message wire frame.

#[cfg(test)]
#[path = "cluster_wire_frame_v1_test.rs"]
mod tests;

use alloc::{borrow::ToOwned, string::String, vec::Vec};

use fraktor_actor_core_kernel_rs::serialization::{SerializedMessage, SerializerId};
use fraktor_cluster_core_kernel_rs::message_serialization::{ClusterMessagePayloadKind, ClusterSerializedMessage};
use postcard::Error;
use serde::{Deserialize, Serialize};

/// Version one cluster message wire frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterWireFrameV1 {
  version:       u16,
  payload_kind:  u16,
  serializer_id: u32,
  manifest:      Option<String>,
  payload_len:   u32,
  payload_bytes: Vec<u8>,
}

impl ClusterWireFrameV1 {
  /// Supported frame version.
  pub const VERSION: u16 = 1;

  /// Creates a version one frame from a cluster serialized message.
  ///
  /// # Errors
  ///
  /// Returns a postcard encode error when the payload length does not fit into
  /// the v1 `u32` length field.
  pub fn try_from_cluster_serialized_message(message: &ClusterSerializedMessage) -> Result<Self, Error> {
    let payload_bytes = message.payload_bytes().to_vec();
    let payload_len = payload_bytes.len().try_into().map_err(|_| Error::SerializeBufferFull)?;
    Ok(Self {
      version: Self::VERSION,
      payload_kind: message.payload_kind().tag(),
      serializer_id: message.serializer_id().value(),
      manifest: message.manifest().map(ToOwned::to_owned),
      payload_len,
      payload_bytes,
    })
  }

  /// Reconstructs the cluster serialized message when the payload kind tag is known.
  #[must_use]
  pub fn to_cluster_serialized_message(&self) -> Option<ClusterSerializedMessage> {
    if self.version != Self::VERSION {
      return None;
    }
    if self.payload_len as usize != self.payload_bytes.len() {
      return None;
    }
    let payload_kind = ClusterMessagePayloadKind::from_tag(self.payload_kind)?;
    let serialized_message = SerializedMessage::new(
      SerializerId::from_raw(self.serializer_id),
      self.manifest.clone(),
      self.payload_bytes.clone(),
    );
    Some(ClusterSerializedMessage::new(payload_kind, serialized_message))
  }

  /// Returns the frame version.
  #[must_use]
  pub const fn version(&self) -> u16 {
    self.version
  }

  /// Returns the raw cluster payload kind tag.
  #[must_use]
  pub const fn payload_kind_tag(&self) -> u16 {
    self.payload_kind
  }

  /// Returns the raw actor-core serializer identifier.
  #[must_use]
  pub const fn serializer_id(&self) -> u32 {
    self.serializer_id
  }

  /// Returns the actor-core manifest.
  #[must_use]
  pub fn manifest(&self) -> Option<&str> {
    self.manifest.as_deref()
  }

  /// Returns the declared payload length.
  #[must_use]
  pub const fn payload_len(&self) -> u32 {
    self.payload_len
  }

  /// Returns the payload bytes.
  #[must_use]
  pub fn payload_bytes(&self) -> &[u8] {
    &self.payload_bytes
  }
}
