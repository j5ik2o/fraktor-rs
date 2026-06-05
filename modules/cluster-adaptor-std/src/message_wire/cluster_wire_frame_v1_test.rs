use alloc::{format, string::String, vec};

use fraktor_actor_core_kernel_rs::serialization::{SerializedMessage, SerializerId};
use fraktor_cluster_core_kernel_rs::message_serialization::{ClusterMessagePayloadKind, ClusterSerializedMessage};

use super::ClusterWireFrameV1;

fn cluster_message() -> ClusterSerializedMessage {
  let serializer_id = SerializerId::try_from(41).expect("serializer id");
  let serialized_message =
    SerializedMessage::new(serializer_id, Some(String::from("cluster.payload/gossip")), vec![1, 1, 2, 3, 5, 8]);
  ClusterSerializedMessage::new(ClusterMessagePayloadKind::Gossip, serialized_message)
}

#[test]
fn from_cluster_serialized_message_preserves_v1_metadata() {
  let message = cluster_message();

  let frame = ClusterWireFrameV1::from_cluster_serialized_message(&message);

  assert_eq!(frame.version(), ClusterWireFrameV1::VERSION);
  assert_eq!(frame.payload_kind_tag(), ClusterMessagePayloadKind::Gossip.tag());
  assert_eq!(frame.serializer_id(), 41);
  assert_eq!(frame.manifest(), Some("cluster.payload/gossip"));
  assert_eq!(frame.payload_len(), 6);
  assert_eq!(frame.payload_bytes(), &[1, 1, 2, 3, 5, 8]);
}

#[test]
fn postcard_roundtrip_preserves_metadata_and_reconstructs_cluster_message() {
  let message = cluster_message();
  let frame = ClusterWireFrameV1::from_cluster_serialized_message(&message);

  let encoded = postcard::to_allocvec(&frame).expect("encode frame");
  let decoded: ClusterWireFrameV1 = postcard::from_bytes(&encoded).expect("decode frame");
  let reconstructed = decoded.to_cluster_serialized_message().expect("known payload kind");

  assert_eq!(decoded, frame);
  assert_eq!(reconstructed.payload_kind(), ClusterMessagePayloadKind::Gossip);
  assert_eq!(reconstructed.serializer_id().value(), 41);
  assert_eq!(reconstructed.manifest(), Some("cluster.payload/gossip"));
  assert_eq!(reconstructed.payload_bytes(), &[1, 1, 2, 3, 5, 8]);
}

#[test]
fn non_v1_frame_is_not_reconstructed() {
  let frame = ClusterWireFrameV1::from_cluster_serialized_message(&cluster_message());
  let mut encoded = postcard::to_allocvec(&frame).expect("encode frame");
  encoded[0] = 2;
  let decoded: ClusterWireFrameV1 = postcard::from_bytes(&encoded).expect("decode frame");

  assert_eq!(decoded.version(), 2);
  assert!(decoded.to_cluster_serialized_message().is_none());
}

#[test]
fn frame_debug_shape_exposes_only_wire_metadata() {
  let frame = ClusterWireFrameV1::from_cluster_serialized_message(&cluster_message());
  let debug = format!("{frame:?}");

  for field in ["version", "payload_kind", "serializer_id", "manifest", "payload_len", "payload_bytes"] {
    assert!(debug.contains(field), "missing field {field} in {debug}");
  }
  for excluded in ["endpoint", "association", "retry"] {
    assert!(!debug.contains(excluded), "unexpected transport field {excluded} in {debug}");
  }
}
