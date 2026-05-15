use alloc::{string::ToString, vec::Vec};

use bytes::{Bytes, BytesMut};

use crate::{
  address::{Address, UniqueAddress},
  wire::{
    AckCodec, AckPdu, Codec, CompressedText, CompressionTableEntry, CompressionTableKind, ControlCodec, ControlPdu,
    EnvelopeCodec, EnvelopePayload, EnvelopePdu, FlushScope, HandshakeCodec, HandshakePdu, HandshakeReq, HandshakeRsp,
    KIND_ACK, KIND_CONTROL, KIND_DEPLOYMENT, KIND_ENVELOPE, KIND_HANDSHAKE_REQ, KIND_HANDSHAKE_RSP,
    RemoteDeploymentCodec, RemoteDeploymentCreateFailure, RemoteDeploymentCreateRequest, RemoteDeploymentCreateSuccess,
    RemoteDeploymentFailureCode, RemoteDeploymentPdu, WIRE_VERSION, WIRE_VERSION_1, WIRE_VERSION_2, WIRE_VERSION_3,
    WIRE_VERSION_4, WireError,
  },
};

fn to_bytes(buf: BytesMut) -> Bytes {
  buf.freeze()
}

fn patch_frame_len(buf: &mut BytesMut) {
  let length = (buf.len() - 4) as u32;
  buf[0..4].copy_from_slice(&length.to_be_bytes());
}

fn control_reason_index(authority: &str) -> usize {
  7 + 4 + authority.len()
}

fn insert_control_reason(buf: &mut BytesMut, authority: &str, reason: &str) {
  let reason_index = control_reason_index(authority);
  buf[reason_index] = 0x01;
  let tail = buf.split_off(reason_index + 1);
  buf.extend_from_slice(&(reason.len() as u32).to_be_bytes());
  buf.extend_from_slice(reason.as_bytes());
  buf.unsplit(tail);
  patch_frame_len(buf);
}

fn remote_deployment_manifest_tag_index(buf: &[u8]) -> usize {
  const FRAME_HEADER_LEN: usize = 6;
  const DEPLOYMENT_KIND_LEN: usize = 1;
  const CORRELATION_HI_LEN: usize = 8;
  const CORRELATION_LO_LEN: usize = 4;
  const STRING_LEN_PREFIX: usize = 4;
  const STRING_FIELD_COUNT_BEFORE_MANIFEST: usize = 4;
  const MANIFEST_TAG_LEN: usize = 4;

  let mut index = FRAME_HEADER_LEN + DEPLOYMENT_KIND_LEN + CORRELATION_HI_LEN + CORRELATION_LO_LEN;
  for _ in 0..STRING_FIELD_COUNT_BEFORE_MANIFEST {
    let len = u32::from_be_bytes([buf[index], buf[index + 1], buf[index + 2], buf[index + 3]]) as usize;
    index += STRING_LEN_PREFIX + len;
  }
  index + MANIFEST_TAG_LEN
}

fn sample_handshake_from() -> UniqueAddress {
  UniqueAddress::new(Address::new("sys", "host", 2552), 0xdead_beef)
}

fn sample_handshake_to() -> Address {
  Address::new("local-sys", "127.0.0.1", 2551)
}

fn test_envelope_pdu(
  recipient_path: String,
  sender_path: Option<String>,
  correlation_hi: u64,
  correlation_lo: u32,
  priority: u8,
  payload: Bytes,
) -> EnvelopePdu {
  let pdu = EnvelopePdu::new(
    recipient_path,
    sender_path,
    correlation_hi,
    correlation_lo,
    priority,
    EnvelopePayload::new(5, None, payload),
  );
  if priority == 0 { pdu.with_redelivery_sequence(Some(100)) } else { pdu }
}

fn test_remote_deployment_request() -> RemoteDeploymentPdu {
  RemoteDeploymentPdu::CreateRequest(RemoteDeploymentCreateRequest::new(
    0x0123_4567_89ab_cdef,
    0xfedc_ba98,
    "/user/parent".to_string(),
    "child".to_string(),
    "echo".to_string(),
    "origin-system@127.0.0.1:2551".to_string(),
    700,
    Some("example.Payload".to_string()),
    Bytes::from_static(b"payload"),
  ))
}

#[test]
fn envelope_roundtrip_with_sender_path() {
  let pdu = EnvelopePdu::new(
    "/user/actor-a".to_string(),
    Some("/user/sender".to_string()),
    0x0123_4567_89ab_cdef,
    0xfedc_ba98,
    1,
    EnvelopePayload::new(7, Some("example.Manifest".to_string()), Bytes::from_static(b"hello")),
  );
  let codec = EnvelopeCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
  assert_eq!(decoded.serializer_id(), 7);
  assert_eq!(decoded.manifest(), Some("example.Manifest"));
  assert_eq!(bytes.len(), 0, "decoder should fully consume the frame");
}

#[test]
fn envelope_roundtrip_without_sender_path() {
  let pdu = test_envelope_pdu("/user/actor-b".to_string(), None, 42, 0, 0, Bytes::from_static(b""));
  let codec = EnvelopeCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
  assert_eq!(decoded.redelivery_sequence(), Some(100));
}

#[test]
fn remote_deployment_create_request_roundtrip() {
  let pdu = test_remote_deployment_request();
  let codec = RemoteDeploymentCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();

  assert_eq!(decoded, pdu);
  match decoded {
    | RemoteDeploymentPdu::CreateRequest(request) => {
      assert_eq!(request.target_parent_path(), "/user/parent");
      assert_eq!(request.child_name(), "child");
      assert_eq!(request.factory_id(), "echo");
      assert_eq!(request.origin_node(), "origin-system@127.0.0.1:2551");
      assert_eq!(request.serializer_id(), 700);
      assert_eq!(request.manifest(), Some("example.Payload"));
      assert_eq!(request.payload(), &Bytes::from_static(b"payload"));
    },
    | _ => panic!("expected create request"),
  }
  assert_eq!(bytes.len(), 0, "decoder should fully consume the frame");
}

#[test]
fn remote_deployment_create_success_roundtrip() {
  let pdu = RemoteDeploymentPdu::CreateSuccess(RemoteDeploymentCreateSuccess::new(
    0x1111_2222_3333_4444,
    0x5555_6666,
    "fraktor.tcp://remote-system@remote.example.com:2552/user/child".to_string(),
  ));
  let mut buf = BytesMut::new();
  RemoteDeploymentCodec::new().encode(&pdu, &mut buf).unwrap();

  let mut bytes = to_bytes(buf);
  let decoded = RemoteDeploymentCodec::new().decode(&mut bytes).unwrap();

  assert_eq!(decoded, pdu);
  assert_eq!(bytes.len(), 0, "decoder should fully consume the frame");
}

#[test]
fn remote_deployment_create_failure_roundtrip() {
  let pdu = RemoteDeploymentPdu::CreateFailure(RemoteDeploymentCreateFailure::new(
    0x1111_2222_3333_4444,
    0x5555_6666,
    RemoteDeploymentFailureCode::UnknownFactoryId,
    "unknown factory id: echo".to_string(),
  ));
  let mut buf = BytesMut::new();
  RemoteDeploymentCodec::new().encode(&pdu, &mut buf).unwrap();

  let mut bytes = to_bytes(buf);
  let decoded = RemoteDeploymentCodec::new().decode(&mut bytes).unwrap();

  assert_eq!(decoded, pdu);
  assert_eq!(bytes.len(), 0, "decoder should fully consume the frame");
}

#[test]
fn remote_deployment_frame_kind_is_distinct_from_envelope() {
  let mut buf = BytesMut::new();
  RemoteDeploymentCodec::new().encode(&test_remote_deployment_request(), &mut buf).unwrap();

  assert_eq!(buf[5], KIND_DEPLOYMENT);
  assert_ne!(buf[5], KIND_ENVELOPE);

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();
  assert_eq!(err, WireError::UnknownKind);
}

#[test]
fn remote_deployment_decode_rejects_invalid_variant_tag() {
  let mut buf = BytesMut::new();
  RemoteDeploymentCodec::new().encode(&test_remote_deployment_request(), &mut buf).unwrap();
  buf[6] = 0xee;

  let err = RemoteDeploymentCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn remote_deployment_decode_rejects_invalid_failure_code() {
  let pdu = RemoteDeploymentPdu::CreateFailure(RemoteDeploymentCreateFailure::new(
    1,
    2,
    RemoteDeploymentFailureCode::Timeout,
    "timeout".to_string(),
  ));
  let mut buf = BytesMut::new();
  RemoteDeploymentCodec::new().encode(&pdu, &mut buf).unwrap();
  buf[19] = 0xee;

  let err = RemoteDeploymentCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn remote_deployment_decode_rejects_invalid_payload_manifest_tag() {
  let mut buf = BytesMut::new();
  RemoteDeploymentCodec::new().encode(&test_remote_deployment_request(), &mut buf).unwrap();
  let manifest_tag_index = remote_deployment_manifest_tag_index(&buf);
  buf[manifest_tag_index] = 0xee;

  let err = RemoteDeploymentCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn remote_deployment_decode_rejects_truncated_payload_metadata() {
  let mut buf = BytesMut::new();
  RemoteDeploymentCodec::new().encode(&test_remote_deployment_request(), &mut buf).unwrap();
  buf.truncate(buf.len() - 2);
  patch_frame_len(&mut buf);

  let err = RemoteDeploymentCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn envelope_frame_kind_is_0x01() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  // layout: [len(4)][version(1)][kind(1)]...
  assert_eq!(buf[5], KIND_ENVELOPE);
}

#[test]
fn envelope_priority_system_is_0x00() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  // layout: [len(4)][version(1)][kind(1)][recipient_tag(1)][recipient_len(4)][recipient...
  // ][sender_tag(1)][corr_hi(8)][corr_lo(4)][priority(1)][payload_len(4)] recipient = "/r" (2
  // bytes), so priority byte is at: 4 + 1 + 1 + 1 + 4 + 2 + 1 + 8 + 4 = 26
  assert_eq!(buf[26], 0x00);
}

#[test]
fn envelope_priority_user_is_0x01() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 1, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[26], 0x01);
}

#[test]
fn envelope_manifest_none_encodes_as_zero_tag_after_serializer_id() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[27], 0x01);
  assert_eq!(&buf[28..36], &100_u64.to_be_bytes());
  assert_eq!(&buf[36..40], &5_u32.to_be_bytes());
  assert_eq!(buf[40], 0x00);
}

#[test]
fn envelope_sender_path_none_encodes_as_zero_tag() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  // After recipient:
  // [len(4)][version(1)][kind(1)][recipient_tag(1)][recipient_len(4)=0x00000002][recipient(2)][sender_tag]
  // sender_tag index = 4 + 1 + 1 + 1 + 4 + 2 = 13
  assert_eq!(buf[13], 0x00);
}

#[test]
fn system_envelope_carries_redelivery_sequence_metadata() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  let mut bytes = to_bytes(buf);
  let decoded = EnvelopeCodec::new().decode(&mut bytes).unwrap();
  assert_eq!(decoded.redelivery_sequence(), Some(100));
}

#[test]
fn user_envelope_omits_redelivery_sequence_metadata() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 1, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  let mut bytes = to_bytes(buf);
  let decoded = EnvelopeCodec::new().decode(&mut bytes).unwrap();
  assert_eq!(decoded.redelivery_sequence(), None);
}

#[test]
fn system_envelope_without_redelivery_sequence_is_rejected() {
  let pdu = EnvelopePdu::new("/r".to_string(), None, 0, 0, 0, EnvelopePayload::new(5, None, Bytes::new()));
  let err = EnvelopeCodec::new().encode(&pdu, &mut BytesMut::new()).unwrap_err();
  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn user_envelope_with_redelivery_sequence_is_rejected() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 1, Bytes::new()).with_redelivery_sequence(Some(10));
  let err = EnvelopeCodec::new().encode(&pdu, &mut BytesMut::new()).unwrap_err();
  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn envelope_decode_rejects_unknown_priority_before_redelivery_metadata() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 1, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf[26] = 0x09;

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn envelope_decode_rejects_unknown_redelivery_flag() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 1, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf[27] = 0x02;

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn envelope_decode_rejects_system_redelivery_flag_without_sequence_bytes() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(32);
  patch_frame_len(&mut buf);

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn envelope_decode_rejects_missing_correlation_metadata() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 1, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(14);
  patch_frame_len(&mut buf);

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn envelope_decode_rejects_missing_serializer_id_after_redelivery_sequence() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(36);
  patch_frame_len(&mut buf);

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn envelope_decode_rejects_system_envelope_without_redelivery_sequence() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf[27] = 0x00;
  let tail = buf.split_off(36);
  buf.truncate(28);
  buf.unsplit(tail);
  patch_frame_len(&mut buf);

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn envelope_decode_rejects_user_envelope_with_redelivery_sequence() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 1, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf[27] = 0x01;
  let tail = buf.split_off(28);
  buf.extend_from_slice(&10_u64.to_be_bytes());
  buf.unsplit(tail);
  patch_frame_len(&mut buf);

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn envelope_actor_path_reference_metadata_roundtrips() {
  let pdu = EnvelopePdu::new_with_metadata(
    CompressedText::table_ref(3),
    Some(CompressedText::literal("/user/sender".to_string())),
    1,
    2,
    1,
    EnvelopePayload::new(7, None, Bytes::from_static(b"hello")),
    None,
  );
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  let decoded = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap();

  assert_eq!(decoded.recipient_path_metadata().as_table_ref(), Some(3));
  assert_eq!(decoded.sender_path_metadata().and_then(CompressedText::as_literal), Some("/user/sender"));
  assert_eq!(decoded.serializer_id(), 7);
  assert_eq!(decoded.payload(), &Bytes::from_static(b"hello"));
}

#[test]
#[should_panic(expected = "recipient_path() called on unresolved compressed table reference")]
fn envelope_recipient_path_panics_for_unresolved_table_ref() {
  let pdu = EnvelopePdu::new_with_metadata(
    CompressedText::table_ref(3),
    None,
    1,
    2,
    1,
    EnvelopePayload::new(7, None, Bytes::from_static(b"hello")),
    None,
  );

  let _ = pdu.recipient_path();
}

#[test]
#[should_panic(expected = "sender_path() called on unresolved compressed table reference")]
fn envelope_sender_path_panics_for_unresolved_table_ref() {
  let pdu = EnvelopePdu::new_with_metadata(
    CompressedText::literal("/user/recipient".to_string()),
    Some(CompressedText::table_ref(3)),
    1,
    2,
    1,
    EnvelopePayload::new(7, None, Bytes::from_static(b"hello")),
    None,
  );

  let _ = pdu.sender_path();
}

#[test]
#[should_panic(expected = "manifest() called on unresolved compressed table reference")]
fn envelope_manifest_panics_for_unresolved_table_ref() {
  let pdu = EnvelopePdu::new_with_metadata(
    CompressedText::literal("/user/recipient".to_string()),
    None,
    1,
    2,
    1,
    EnvelopePayload::new(7, None, Bytes::from_static(b"hello")),
    Some(CompressedText::table_ref(5)),
  );

  let _ = pdu.manifest();
}

#[test]
fn envelope_manifest_reference_metadata_roundtrips_without_payload_compression() {
  let pdu = EnvelopePdu::new_with_metadata(
    CompressedText::literal("/user/recipient".to_string()),
    None,
    1,
    2,
    1,
    EnvelopePayload::new(7, None, Bytes::from_static(b"hello")),
    Some(CompressedText::table_ref(5)),
  );
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  let decoded = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap();

  assert_eq!(decoded.manifest_metadata().and_then(CompressedText::as_table_ref), Some(5));
  assert_eq!(decoded.serializer_id(), 7);
  assert_eq!(decoded.payload(), &Bytes::from_static(b"hello"));
}

#[test]
fn handshake_req_roundtrip() {
  let from = sample_handshake_from();
  let to = sample_handshake_to();
  let pdu = HandshakePdu::Req(HandshakeReq::new(from.clone(), to.clone()));
  let codec = HandshakeCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[5], KIND_HANDSHAKE_REQ);
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
  assert!(matches!(
    decoded,
    HandshakePdu::Req(req) if req.from() == &from && req.to() == &to
  ));
}

#[test]
fn handshake_rsp_roundtrip() {
  let from = sample_handshake_from();
  let pdu = HandshakePdu::Rsp(HandshakeRsp::new(from.clone()));
  let codec = HandshakeCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[5], KIND_HANDSHAKE_RSP);
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
  assert!(matches!(decoded, HandshakePdu::Rsp(rsp) if rsp.from() == &from));
}

#[test]
fn control_heartbeat_roundtrip() {
  let pdu = ControlPdu::Heartbeat { authority: "sys@host:1".to_string() };
  let codec = ControlCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[5], KIND_CONTROL);
  assert_eq!(buf[6], 0x00, "subkind for heartbeat should be 0x00");
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
}

#[test]
fn control_heartbeat_response_roundtrip_with_uid() {
  let pdu = ControlPdu::HeartbeatResponse { authority: "sys@host:1".to_string(), uid: 0x0102_0304_0506_0708 };
  let codec = ControlCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[5], KIND_CONTROL);
  assert_eq!(buf[6], 0x03, "subkind for heartbeat response should be 0x03");
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
}

#[test]
fn control_heartbeat_response_rejects_truncated_uid() {
  let pdu = ControlPdu::HeartbeatResponse { authority: "sys@host:1".to_string(), uid: 0x0102_0304_0506_0708 };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(buf.len() - 1);
  patch_frame_len(&mut buf);

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn control_quarantine_roundtrip_with_reason() {
  let pdu = ControlPdu::Quarantine { authority: "sys@host:2".to_string(), reason: Some("timed out".to_string()) };
  let codec = ControlCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[6], 0x01, "subkind for quarantine should be 0x01");
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
}

#[test]
fn control_shutdown_roundtrip() {
  let pdu = ControlPdu::Shutdown { authority: "sys@host:3".to_string() };
  let codec = ControlCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[6], 0x02, "subkind for shutdown should be 0x02");
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
}

#[test]
fn control_flush_request_roundtrip() {
  let pdu = ControlPdu::FlushRequest {
    authority:     "sys@host:4".to_string(),
    flush_id:      42,
    scope:         FlushScope::Shutdown,
    lane_id:       3,
    expected_acks: 2,
  };
  let codec = ControlCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[5], KIND_CONTROL);
  assert_eq!(buf[6], 0x04, "subkind for flush request should be 0x04");
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
  assert_eq!(bytes.len(), 0, "decoder should fully consume the frame");
}

#[test]
fn control_flush_request_roundtrips_before_deathwatch_scope() {
  let pdu = ControlPdu::FlushRequest {
    authority:     "sys@host:4".to_string(),
    flush_id:      42,
    scope:         FlushScope::BeforeDeathWatchNotification,
    lane_id:       3,
    expected_acks: 2,
  };
  let codec = ControlCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  let mut bytes = to_bytes(buf);

  let decoded = codec.decode(&mut bytes).unwrap();

  assert_eq!(decoded, pdu);
}

#[test]
fn control_flush_ack_roundtrip() {
  let pdu = ControlPdu::FlushAck {
    authority:     "sys@host:5".to_string(),
    flush_id:      43,
    lane_id:       3,
    expected_acks: 2,
  };
  let codec = ControlCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[5], KIND_CONTROL);
  assert_eq!(buf[6], 0x05, "subkind for flush ack should be 0x05");
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
  assert_eq!(bytes.len(), 0, "decoder should fully consume the frame");
}

#[test]
fn control_decode_rejects_missing_subkind() {
  let mut buf = BytesMut::new();
  buf.extend_from_slice(&2_u32.to_be_bytes());
  buf.extend_from_slice(&[WIRE_VERSION, KIND_CONTROL]);

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn control_decode_rejects_unknown_subkind() {
  let pdu = ControlPdu::Heartbeat { authority: "sys@host:1".to_string() };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  buf[6] = 0xff;

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn control_compression_advertisement_roundtrip() {
  let pdu = ControlPdu::CompressionAdvertisement {
    authority:  "sys@host:6".to_string(),
    table_kind: CompressionTableKind::ActorRef,
    generation: 7,
    entries:    vec![CompressionTableEntry::new(3, "/user/a".to_string())],
  };
  let codec = ControlCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[5], KIND_CONTROL);
  assert_eq!(buf[6], 0x06, "subkind for compression advertisement should be 0x06");
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
  assert_eq!(bytes.len(), 0, "decoder should fully consume the frame");
}

#[test]
fn control_compression_advertisement_rejects_reason_field() {
  let authority = "sys@host:6".to_string();
  let pdu = ControlPdu::CompressionAdvertisement {
    authority:  authority.clone(),
    table_kind: CompressionTableKind::ActorRef,
    generation: 7,
    entries:    vec![CompressionTableEntry::new(3, "/user/a".to_string())],
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  insert_control_reason(&mut buf, &authority, "bad");

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn control_compression_ack_roundtrip() {
  let pdu = ControlPdu::CompressionAck {
    authority:  "sys@host:7".to_string(),
    table_kind: CompressionTableKind::Manifest,
    generation: 8,
  };
  let codec = ControlCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[5], KIND_CONTROL);
  assert_eq!(buf[6], 0x07, "subkind for compression ack should be 0x07");
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
  assert_eq!(bytes.len(), 0, "decoder should fully consume the frame");
}

#[test]
fn control_compression_ack_rejects_reason_field() {
  let authority = "sys@host:7".to_string();
  let pdu = ControlPdu::CompressionAck {
    authority:  authority.clone(),
    table_kind: CompressionTableKind::Manifest,
    generation: 8,
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  insert_control_reason(&mut buf, &authority, "bad");

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn control_compression_ack_rejects_truncated_body() {
  let pdu = ControlPdu::CompressionAck {
    authority:  "sys@host:7".to_string(),
    table_kind: CompressionTableKind::Manifest,
    generation: 8,
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(buf.len() - 1);
  patch_frame_len(&mut buf);

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn control_compression_advertisement_rejects_unknown_table_kind() {
  let authority = "sys@host:6".to_string();
  let pdu = ControlPdu::CompressionAdvertisement {
    authority:  authority.clone(),
    table_kind: CompressionTableKind::ActorRef,
    generation: 7,
    entries:    vec![CompressionTableEntry::new(3, "/user/a".to_string())],
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  let table_kind_index = 7 + 4 + authority.len() + 1;
  buf[table_kind_index] = 0xff;

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn control_compression_advertisement_rejects_missing_entry_id() {
  let authority = "sys@host:6".to_string();
  let pdu = ControlPdu::CompressionAdvertisement {
    authority:  authority.clone(),
    table_kind: CompressionTableKind::ActorRef,
    generation: 7,
    entries:    Vec::new(),
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  let entry_count_index = control_reason_index(&authority) + 1 + 1 + 8;
  buf[entry_count_index..entry_count_index + 4].copy_from_slice(&1_u32.to_be_bytes());

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn control_compression_advertisement_rejects_entry_count_exceeding_remaining_bytes() {
  let authority = "sys@host:6".to_string();
  let pdu = ControlPdu::CompressionAdvertisement {
    authority:  authority.clone(),
    table_kind: CompressionTableKind::ActorRef,
    generation: 7,
    entries:    Vec::new(),
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  let entry_count_index = control_reason_index(&authority) + 1 + 1 + 8;
  buf[entry_count_index..entry_count_index + 4].copy_from_slice(&u32::MAX.to_be_bytes());

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn control_compression_advertisement_rejects_missing_entry_count() {
  let pdu = ControlPdu::CompressionAdvertisement {
    authority:  "sys@host:6".to_string(),
    table_kind: CompressionTableKind::ActorRef,
    generation: 7,
    entries:    Vec::new(),
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(buf.len() - 4);
  patch_frame_len(&mut buf);

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn control_compression_advertisement_rejects_truncated_entry() {
  let pdu = ControlPdu::CompressionAdvertisement {
    authority:  "sys@host:6".to_string(),
    table_kind: CompressionTableKind::ActorRef,
    generation: 7,
    entries:    vec![CompressionTableEntry::new(3, "/user/a".to_string())],
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(buf.len() - 1);
  patch_frame_len(&mut buf);

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn control_flush_request_rejects_truncated_body() {
  let pdu = ControlPdu::FlushRequest {
    authority:     "sys@host:4".to_string(),
    flush_id:      42,
    scope:         FlushScope::Shutdown,
    lane_id:       3,
    expected_acks: 2,
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(buf.len() - 1);
  patch_frame_len(&mut buf);

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn control_flush_ack_rejects_truncated_body() {
  let pdu = ControlPdu::FlushAck {
    authority:     "sys@host:5".to_string(),
    flush_id:      43,
    lane_id:       3,
    expected_acks: 2,
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(buf.len() - 1);
  patch_frame_len(&mut buf);

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn control_flush_request_rejects_unknown_scope() {
  let authority = "sys@host:4".to_string();
  let pdu = ControlPdu::FlushRequest {
    authority:     authority.clone(),
    flush_id:      42,
    scope:         FlushScope::BeforeDeathWatchNotification,
    lane_id:       3,
    expected_acks: 2,
  };
  let mut buf = BytesMut::new();
  ControlCodec::new().encode(&pdu, &mut buf).unwrap();
  let scope_index = 7 + 4 + authority.len() + 1 + 8;
  buf[scope_index] = 0x09;

  let err = ControlCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn ack_roundtrip() {
  let pdu = AckPdu::new(100, 99, 0b0110);
  let codec = AckCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[5], KIND_ACK);
  let mut bytes = to_bytes(buf);
  let decoded = codec.decode(&mut bytes).unwrap();
  assert_eq!(decoded, pdu);
}

#[test]
fn unknown_version_byte_is_rejected() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let codec = EnvelopeCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  // Flip the version byte.
  buf[4] = 0xFF;
  let mut bytes = to_bytes(buf);
  let err = codec.decode(&mut bytes).unwrap_err();
  assert_eq!(err, WireError::UnknownVersion);
}

#[test]
fn previous_wire_version_byte_is_rejected() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let codec = EnvelopeCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  buf[4] = WIRE_VERSION_2;
  let mut bytes = to_bytes(buf);
  let err = codec.decode(&mut bytes).unwrap_err();
  assert_eq!(err, WireError::UnknownVersion);
}

#[test]
fn unknown_kind_byte_is_rejected() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let codec = EnvelopeCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  // Flip the kind byte to an undefined value (decoding as envelope must fail).
  buf[5] = 0xEE;
  let mut bytes = to_bytes(buf);
  let err = codec.decode(&mut bytes).unwrap_err();
  assert_eq!(err, WireError::UnknownKind);
}

#[test]
fn truncated_buffer_is_rejected() {
  let pdu = test_envelope_pdu("/user/r".to_string(), Some("/user/s".to_string()), 77, 0, 1, Bytes::from_static(b"xyz"));
  let codec = EnvelopeCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  // Drop the last byte to truncate the frame.
  let bytes_vec: Vec<u8> = buf.to_vec();
  let mut bytes = Bytes::copy_from_slice(&bytes_vec[..bytes_vec.len() - 1]);
  let err = codec.decode(&mut bytes).unwrap_err();
  assert_eq!(err, WireError::Truncated);
}

#[test]
fn oversized_length_field_is_rejected() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let codec = EnvelopeCodec::new();
  let mut buf = BytesMut::new();
  codec.encode(&pdu, &mut buf).unwrap();
  // Patch the length field to a value larger than the remaining buffer.
  let huge = u32::MAX.to_be_bytes();
  buf[0..4].copy_from_slice(&huge);
  let mut bytes = to_bytes(buf);
  let err = codec.decode(&mut bytes).unwrap_err();
  assert!(matches!(err, WireError::Truncated | WireError::InvalidFormat));
}

#[test]
fn all_kinds_are_distinct() {
  let kinds = [KIND_ENVELOPE, KIND_HANDSHAKE_REQ, KIND_HANDSHAKE_RSP, KIND_CONTROL, KIND_ACK, KIND_DEPLOYMENT];
  for (i, &a) in kinds.iter().enumerate() {
    for &b in &kinds[i + 1..] {
      assert_ne!(a, b, "kind {a:#04x} collides with {b:#04x}");
    }
  }
}

#[test]
fn wire_version_byte_is_current() {
  assert_eq!(WIRE_VERSION_1, 0x01);
  assert_eq!(WIRE_VERSION_2, 0x02);
  assert_eq!(WIRE_VERSION_3, 0x03);
  assert_eq!(WIRE_VERSION_4, 0x04);
  assert_eq!(WIRE_VERSION, WIRE_VERSION_3);
}

#[test]
fn string_hello_has_expected_bytes() {
  // Encode an envelope with a known-size recipient path so we can inspect the
  // length-prefixed string bytes for the spec's canonical example (`"hello"`).
  let pdu = test_envelope_pdu("hello".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  // layout: [len(4)][version(1)][kind(1)][recipient_tag(1)][recipient_len(4)][recipient_bytes...]
  // recipient_len starts at offset 7. For "hello" (5 bytes):
  assert_eq!(buf[6], 0x00);
  assert_eq!(&buf[7..11], &[0x00, 0x00, 0x00, 0x05]);
  assert_eq!(&buf[11..16], b"hello");
}

#[test]
fn invalid_utf8_in_string_is_rejected() {
  // Manually construct an envelope frame whose recipient_path carries an
  // invalid UTF-8 byte sequence.
  let mut body = BytesMut::new();
  // recipient compressed text: literal tag, length 3, bytes 0xFF 0xFE 0xFD (invalid UTF-8)
  body.extend_from_slice(&[0x00]);
  body.put_u32_len_prefix_invalid(3);
  body.extend_from_slice(&[0xFF, 0xFE, 0xFD]);
  // sender_path: None
  body.extend_from_slice(&[0x00]);
  // correlation_id: hi(u64) + lo(u32) = 96 bits
  body.extend_from_slice(&0u64.to_be_bytes());
  body.extend_from_slice(&0u32.to_be_bytes());
  // priority: u8
  body.extend_from_slice(&[0x00]);
  // redelivery_sequence: Some(0) for system priority
  body.extend_from_slice(&[0x01]);
  body.extend_from_slice(&0u64.to_be_bytes());
  // serializer_id: Vec<u8> built-in id
  body.extend_from_slice(&5_u32.to_be_bytes());
  // manifest: None
  body.extend_from_slice(&[0x00]);
  // payload: empty (length 0)
  body.extend_from_slice(&0u32.to_be_bytes());

  let mut frame = BytesMut::new();
  let length = (2 + body.len()) as u32;
  frame.extend_from_slice(&length.to_be_bytes());
  frame.extend_from_slice(&[WIRE_VERSION, KIND_ENVELOPE]);
  frame.extend_from_slice(&body);

  let mut bytes = frame.freeze();
  let err = EnvelopeCodec::new().decode(&mut bytes).unwrap_err();
  assert_eq!(err, WireError::InvalidUtf8);
}

// Helper extension that the invalid-utf8 test uses to write a raw length-prefix
// without requiring the full primitives encoding path.
trait BytesMutExt {
  fn put_u32_len_prefix_invalid(&mut self, len: u32);
}

impl BytesMutExt for BytesMut {
  fn put_u32_len_prefix_invalid(&mut self, len: u32) {
    self.extend_from_slice(&len.to_be_bytes());
  }
}
