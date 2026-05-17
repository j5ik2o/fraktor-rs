use alloc::{string::String, vec};
use core::{any::TypeId, num::NonZeroU64};

use fraktor_actor_core_kernel_rs::serialization::{
  SerializationError, SerializedMessage, Serializer, SerializerId, SerializerWithStringManifest,
};

use super::{
  ACK_MANIFEST, CUMULATIVE_DEMAND_MANIFEST, ON_SUBSCRIBE_HANDSHAKE_MANIFEST, REMOTE_STREAM_COMPLETED_MANIFEST,
  REMOTE_STREAM_FAILURE_MANIFEST, SEQUENCED_ON_NEXT_MANIFEST, SINK_REF_MANIFEST, SOURCE_REF_MANIFEST,
  STREAM_REF_PROTOCOL_SERIALIZER_ID, StreamRefProtocolSerializer,
};
use crate::stream_ref::{
  StreamRefAck, StreamRefCumulativeDemand, StreamRefOnSubscribeHandshake, StreamRefRemoteStreamCompleted,
  StreamRefRemoteStreamFailure, StreamRefSequencedOnNext, StreamRefSinkRefPayload, StreamRefSourceRefPayload,
};

fn serializer() -> StreamRefProtocolSerializer {
  StreamRefProtocolSerializer::new(STREAM_REF_PROTOCOL_SERIALIZER_ID)
}

#[test]
fn sequenced_on_next_round_trips_serialized_payload() {
  let serializer = serializer();
  let nested = SerializedMessage::new(SerializerId::from_raw(700), Some(String::from("payload.Manifest")), vec![1, 2]);
  let message = StreamRefSequencedOnNext::new(3, nested.clone());

  let bytes = serializer.to_binary(&message).expect("serialize sequenced payload");
  let decoded_by_type =
    serializer.from_binary(&bytes, Some(TypeId::of::<StreamRefSequencedOnNext>())).expect("deserialize by type");
  let decoded =
    serializer.from_binary_with_manifest(&bytes, SEQUENCED_ON_NEXT_MANIFEST).expect("deserialize sequenced payload");

  let decoded_by_type = decoded_by_type.downcast::<StreamRefSequencedOnNext>().expect("sequenced payload by type");
  let decoded = decoded.downcast::<StreamRefSequencedOnNext>().expect("sequenced payload");
  assert_eq!(serializer.manifest(&message), SEQUENCED_ON_NEXT_MANIFEST);
  assert_eq!(decoded_by_type.seq_nr(), 3);
  assert_eq!(decoded_by_type.payload(), &nested);
  assert_eq!(decoded.seq_nr(), 3);
  assert_eq!(decoded.payload(), &nested);
}

#[test]
fn cumulative_demand_round_trips_non_zero_demand() {
  let serializer = serializer();
  let demand = NonZeroU64::new(5).expect("non-zero demand");
  let message = StreamRefCumulativeDemand::new(2, demand);

  let bytes = serializer.to_binary(&message).expect("serialize demand");
  let decoded =
    serializer.from_binary(&bytes, Some(TypeId::of::<StreamRefCumulativeDemand>())).expect("deserialize demand");

  let decoded = decoded.downcast::<StreamRefCumulativeDemand>().expect("demand payload");
  assert_eq!(serializer.manifest(&message), CUMULATIVE_DEMAND_MANIFEST);
  assert_eq!(decoded.seq_nr(), 2);
  assert_eq!(decoded.demand(), demand);
}

#[test]
fn cumulative_demand_rejects_zero_demand_wire_value() {
  let serializer = serializer();
  let mut bytes = Vec::new();
  bytes.extend_from_slice(&1_u64.to_le_bytes());
  bytes.extend_from_slice(&0_u64.to_le_bytes());

  let error =
    serializer.from_binary_with_manifest(&bytes, CUMULATIVE_DEMAND_MANIFEST).expect_err("zero demand should fail");

  assert_eq!(error, SerializationError::InvalidFormat);
}

#[test]
fn handshake_completion_failure_and_ack_round_trip() {
  let serializer = serializer();
  let handshake = StreamRefOnSubscribeHandshake::new(String::from("fraktor://sys@127.0.0.1:2552/user/ref"));
  let completed = StreamRefRemoteStreamCompleted::new(9);
  let failure = StreamRefRemoteStreamFailure::new(String::from("boom"));
  let ack = StreamRefAck;

  let handshake_bytes = serializer.to_binary(&handshake).expect("serialize handshake");
  let completed_bytes = serializer.to_binary(&completed).expect("serialize completion");
  let failure_bytes = serializer.to_binary(&failure).expect("serialize failure");
  let ack_bytes = serializer.to_binary(&ack).expect("serialize ack");

  let decoded_handshake = serializer
    .from_binary_with_manifest(&handshake_bytes, ON_SUBSCRIBE_HANDSHAKE_MANIFEST)
    .expect("decode handshake")
    .downcast::<StreamRefOnSubscribeHandshake>()
    .expect("handshake");
  let decoded_completed = serializer
    .from_binary_with_manifest(&completed_bytes, REMOTE_STREAM_COMPLETED_MANIFEST)
    .expect("decode completed")
    .downcast::<StreamRefRemoteStreamCompleted>()
    .expect("completed");
  let decoded_failure = serializer
    .from_binary_with_manifest(&failure_bytes, REMOTE_STREAM_FAILURE_MANIFEST)
    .expect("decode failure")
    .downcast::<StreamRefRemoteStreamFailure>()
    .expect("failure");
  let decoded_ack = serializer
    .from_binary_with_manifest(&ack_bytes, ACK_MANIFEST)
    .expect("decode ack")
    .downcast::<StreamRefAck>()
    .expect("ack");
  let typed_handshake = serializer
    .from_binary(&handshake_bytes, Some(TypeId::of::<StreamRefOnSubscribeHandshake>()))
    .expect("typed handshake")
    .downcast::<StreamRefOnSubscribeHandshake>()
    .expect("typed handshake");
  let typed_completed = serializer
    .from_binary(&completed_bytes, Some(TypeId::of::<StreamRefRemoteStreamCompleted>()))
    .expect("typed completed")
    .downcast::<StreamRefRemoteStreamCompleted>()
    .expect("typed completed");
  let typed_failure = serializer
    .from_binary(&failure_bytes, Some(TypeId::of::<StreamRefRemoteStreamFailure>()))
    .expect("typed failure")
    .downcast::<StreamRefRemoteStreamFailure>()
    .expect("typed failure");
  let typed_ack = serializer
    .from_binary(&ack_bytes, Some(TypeId::of::<StreamRefAck>()))
    .expect("typed ack")
    .downcast::<StreamRefAck>()
    .expect("typed ack");

  assert_eq!(serializer.manifest(&handshake), ON_SUBSCRIBE_HANDSHAKE_MANIFEST);
  assert_eq!(serializer.manifest(&completed), REMOTE_STREAM_COMPLETED_MANIFEST);
  assert_eq!(serializer.manifest(&failure), REMOTE_STREAM_FAILURE_MANIFEST);
  assert_eq!(serializer.manifest(&ack), ACK_MANIFEST);
  assert_eq!(decoded_handshake.target_ref_path(), handshake.target_ref_path());
  assert_eq!(decoded_completed.seq_nr(), completed.seq_nr());
  assert_eq!(decoded_failure.message(), failure.message());
  assert_eq!(*decoded_ack, StreamRefAck);
  assert_eq!(typed_handshake.target_ref_path(), handshake.target_ref_path());
  assert_eq!(typed_completed.seq_nr(), completed.seq_nr());
  assert_eq!(typed_failure.message(), failure.message());
  assert_eq!(*typed_ack, StreamRefAck);
}

#[test]
fn source_ref_and_sink_ref_payloads_round_trip_actor_paths() {
  let serializer = serializer();
  let source_ref = StreamRefSourceRefPayload::new(String::from("fraktor://sys@127.0.0.1:2552/user/source"));
  let sink_ref = StreamRefSinkRefPayload::new(String::from("fraktor://sys@127.0.0.1:2552/user/sink"));

  let source_bytes = serializer.to_binary(&source_ref).expect("serialize source ref payload");
  let sink_bytes = serializer.to_binary(&sink_ref).expect("serialize sink ref payload");
  let decoded_source = serializer
    .from_binary_with_manifest(&source_bytes, SOURCE_REF_MANIFEST)
    .expect("decode source ref payload")
    .downcast::<StreamRefSourceRefPayload>()
    .expect("source ref payload");
  let decoded_sink = serializer
    .from_binary_with_manifest(&sink_bytes, SINK_REF_MANIFEST)
    .expect("decode sink ref payload")
    .downcast::<StreamRefSinkRefPayload>()
    .expect("sink ref payload");
  let typed_source = serializer
    .from_binary(&source_bytes, Some(TypeId::of::<StreamRefSourceRefPayload>()))
    .expect("typed source ref payload")
    .downcast::<StreamRefSourceRefPayload>()
    .expect("typed source ref payload");
  let typed_sink = serializer
    .from_binary(&sink_bytes, Some(TypeId::of::<StreamRefSinkRefPayload>()))
    .expect("typed sink ref payload")
    .downcast::<StreamRefSinkRefPayload>()
    .expect("typed sink ref payload");

  assert_eq!(serializer.manifest(&source_ref), SOURCE_REF_MANIFEST);
  assert_eq!(serializer.manifest(&sink_ref), SINK_REF_MANIFEST);
  assert_eq!(decoded_source.actor_path(), source_ref.actor_path());
  assert_eq!(decoded_sink.actor_path(), sink_ref.actor_path());
  assert_eq!(typed_source.actor_path(), source_ref.actor_path());
  assert_eq!(typed_sink.actor_path(), sink_ref.actor_path());
}

#[test]
fn unknown_manifest_returns_unknown_manifest() {
  let serializer = serializer();

  let error = serializer.from_binary_with_manifest(&[], "missing").expect_err("unknown manifest should fail");

  assert_eq!(error, SerializationError::UnknownManifest(String::from("missing")));
}

#[test]
fn serializer_reports_manifest_support_and_identity() {
  let serializer = serializer();

  assert_eq!(serializer.identifier(), STREAM_REF_PROTOCOL_SERIALIZER_ID);
  assert!(serializer.include_manifest());
  assert!(serializer.as_any().is::<StreamRefProtocolSerializer>());
  assert!(serializer.as_string_manifest().is_some());
}

#[test]
fn unsupported_message_and_type_hint_are_rejected() {
  let serializer = serializer();

  let binary_error = serializer.to_binary(&7_u32).expect_err("unsupported message type");
  let missing_hint_error = serializer.from_binary(&[], None).expect_err("missing type hint");
  let unsupported_hint_error =
    serializer.from_binary(&[], Some(TypeId::of::<u32>())).expect_err("unsupported type hint");

  assert_eq!(binary_error, SerializationError::InvalidFormat);
  assert_eq!(missing_hint_error, SerializationError::InvalidFormat);
  assert_eq!(unsupported_hint_error, SerializationError::InvalidFormat);
  assert_eq!(serializer.manifest(&7_u32), "");
}

#[test]
fn malformed_wire_values_are_rejected() {
  let serializer = serializer();

  let short_len_prefix =
    serializer.from_binary_with_manifest(&[0, 0, 0], ON_SUBSCRIBE_HANDSHAKE_MANIFEST).expect_err("short length prefix");
  let short_u64 = serializer.from_binary_with_manifest(&[0], REMOTE_STREAM_COMPLETED_MANIFEST).expect_err("short u64");
  let overlong_string = serializer
    .from_binary_with_manifest(&[3, 0, 0, 0, b'a'], REMOTE_STREAM_FAILURE_MANIFEST)
    .expect_err("overlong string");
  let non_empty_ack = serializer.from_binary_with_manifest(&[1], ACK_MANIFEST).expect_err("ack should be empty");

  assert_eq!(short_len_prefix, SerializationError::InvalidFormat);
  assert_eq!(short_u64, SerializationError::InvalidFormat);
  assert_eq!(overlong_string, SerializationError::InvalidFormat);
  assert_eq!(non_empty_ack, SerializationError::InvalidFormat);
}
