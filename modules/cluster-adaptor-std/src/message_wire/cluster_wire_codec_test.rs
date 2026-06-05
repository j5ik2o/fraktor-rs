use alloc::{string::String, vec, vec::Vec};

use fraktor_actor_core_kernel_rs::serialization::{SerializedMessage, SerializerId};
use fraktor_cluster_core_kernel_rs::message_serialization::{ClusterMessagePayloadKind, ClusterSerializedMessage};
use postcard::to_allocvec;

use super::ClusterWireCodec;
use crate::message_wire::{ClusterWireDecodeFailure, ClusterWireFrameV1};

fn cluster_message_with_manifest(manifest: Option<String>, payload_bytes: Vec<u8>) -> ClusterSerializedMessage {
  let serialized_message = SerializedMessage::new(SerializerId::from_raw(41), manifest, payload_bytes);
  ClusterSerializedMessage::new(ClusterMessagePayloadKind::Gossip, serialized_message)
}

fn cluster_message() -> ClusterSerializedMessage {
  cluster_message_with_manifest(Some(String::from("cluster.payload/gossip")), vec![1, 1, 2, 3, 5, 8])
}

fn encoded_frame(message: &ClusterSerializedMessage) -> Vec<u8> {
  let frame = ClusterWireFrameV1::from_cluster_serialized_message(message);
  to_allocvec(&frame).expect("encode frame")
}

#[test]
fn decode_roundtrip_preserves_cluster_serialized_metadata() {
  let codec = ClusterWireCodec;
  let message = cluster_message();

  let encoded = codec.encode(&message).expect("encode message");
  let decoded = codec.decode(&encoded).expect("decode message");

  assert_eq!(decoded.payload_kind(), ClusterMessagePayloadKind::Gossip);
  assert_eq!(decoded.serializer_id().value(), 41);
  assert_eq!(decoded.manifest(), Some("cluster.payload/gossip"));
  assert_eq!(decoded.payload_bytes(), &[1, 1, 2, 3, 5, 8]);
}

#[test]
fn unsupported_frame_version_returns_unknown_version() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message());
  encoded[0] = 2;

  let failure = codec.decode(&encoded).expect_err("unknown version");

  assert_eq!(failure, ClusterWireDecodeFailure::UnknownVersion);
}

#[test]
fn unknown_payload_kind_tag_returns_unknown_payload_kind() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message());
  encoded[1] = 99;

  let failure = codec.decode(&encoded).expect_err("unknown payload kind");

  assert_eq!(failure, ClusterWireDecodeFailure::UnknownPayloadKind);
}

#[test]
fn payload_length_mismatch_returns_malformed_payload() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message_with_manifest(None, vec![1, 2, 3]));
  encoded[4] = 4;

  let failure = codec.decode(&encoded).expect_err("payload length mismatch");

  assert_eq!(failure, ClusterWireDecodeFailure::MalformedPayload);
}

#[test]
fn invalid_manifest_bytes_returns_malformed_payload() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message_with_manifest(Some(String::from("a")), Vec::new()));
  let manifest_byte = encoded.iter().position(|byte| *byte == b'a').expect("manifest byte");
  encoded[manifest_byte] = 0xff;

  let failure = codec.decode(&encoded).expect_err("invalid manifest bytes");

  assert_eq!(failure, ClusterWireDecodeFailure::MalformedPayload);
}

#[test]
fn invalid_postcard_bytes_return_malformed_payload() {
  let codec = ClusterWireCodec;
  let encoded = [0xff];

  let failure = codec.decode(&encoded).expect_err("invalid postcard bytes");

  assert_eq!(failure, ClusterWireDecodeFailure::MalformedPayload);
}

#[test]
fn trailing_bytes_return_malformed_payload() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message());
  encoded.extend_from_slice(&[0x01, 0x02]);

  let failure = codec.decode(&encoded).expect_err("trailing bytes");

  assert_eq!(failure, ClusterWireDecodeFailure::MalformedPayload);
}

#[test]
fn decode_failure_returns_error_without_fallback_message() {
  let codec = ClusterWireCodec;
  let mut encoded = encoded_frame(&cluster_message());
  encoded[1] = 99;

  let result = codec.decode(&encoded);

  assert!(matches!(result, Err(ClusterWireDecodeFailure::UnknownPayloadKind)));
}
