//! Cluster message wire codec.

#[cfg(test)]
#[path = "cluster_wire_codec_test.rs"]
mod tests;

use alloc::{borrow::ToOwned, vec::Vec};

use fraktor_actor_core_kernel_rs::serialization::{SerializedMessage, SerializerId};
use fraktor_cluster_core_kernel_rs::message_serialization::{ClusterMessagePayloadKind, ClusterSerializedMessage};
use postcard::{Error, take_from_bytes, to_allocvec};

use super::{ClusterWireDecodeFailure, ClusterWireFrameV1};

/// Encodes and decodes cluster serialized messages at the std wire boundary.
pub struct ClusterWireCodec;

impl ClusterWireCodec {
  /// Encodes a cluster serialized message into a versioned wire frame.
  ///
  /// # Errors
  ///
  /// Returns a postcard encode error when the frame cannot be serialized.
  pub fn encode(&self, message: &ClusterSerializedMessage) -> Result<Vec<u8>, Error> {
    let frame = ClusterWireFrameV1::try_from_cluster_serialized_message(message)?;
    to_allocvec(&frame)
  }

  /// Decodes a versioned wire frame into a cluster serialized message.
  ///
  /// # Errors
  ///
  /// Returns a typed decode failure when the frame is unsupported or malformed.
  pub fn decode(&self, bytes: &[u8]) -> Result<ClusterSerializedMessage, ClusterWireDecodeFailure> {
    let (frame, remainder): (ClusterWireFrameV1, &[u8]) =
      take_from_bytes(bytes).map_err(|_| ClusterWireDecodeFailure::MalformedPayload)?;
    if frame.version() != ClusterWireFrameV1::VERSION {
      return Err(ClusterWireDecodeFailure::UnknownVersion);
    }
    if !remainder.is_empty() {
      return Err(ClusterWireDecodeFailure::MalformedPayload);
    }
    let payload_kind = ClusterMessagePayloadKind::from_tag(frame.payload_kind_tag())
      .ok_or(ClusterWireDecodeFailure::UnknownPayloadKind)?;
    if frame.payload_len() as usize != frame.payload_bytes().len() {
      return Err(ClusterWireDecodeFailure::MalformedPayload);
    }
    let serialized_message = SerializedMessage::new(
      SerializerId::from_raw(frame.serializer_id()),
      frame.manifest().map(ToOwned::to_owned),
      frame.payload_bytes().to_vec(),
    );
    Ok(ClusterSerializedMessage::new(payload_kind, serialized_message))
  }
}
