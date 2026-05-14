use alloc::{string::ToString, vec::Vec};

use bytes::{Bytes, BytesMut};

use crate::{
  address::{Address, UniqueAddress},
  wire::{
    AckCodec, AckPdu, Codec, ControlCodec, ControlPdu, EnvelopeCodec, EnvelopePayload, EnvelopePdu, HandshakeCodec,
    HandshakePdu, HandshakeReq, HandshakeRsp, KIND_ACK, KIND_CONTROL, KIND_ENVELOPE, KIND_HANDSHAKE_REQ,
    KIND_HANDSHAKE_RSP, WIRE_VERSION, WIRE_VERSION_1, WIRE_VERSION_2, WireError,
  },
};

fn to_bytes(buf: BytesMut) -> Bytes {
  buf.freeze()
}

fn patch_frame_len(buf: &mut BytesMut) {
  let length = (buf.len() - 4) as u32;
  buf[0..4].copy_from_slice(&length.to_be_bytes());
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
  // layout: [len(4)][version(1)][kind(1)][recipient_len(4)][recipient...
  // ][sender_tag(1)][corr_hi(8)][corr_lo(4)][priority(1)][payload_len(4)] recipient = "/r" (2
  // bytes), so priority byte is at: 4 + 1 + 1 + 4 + 2 + 1 + 8 + 4 = 25
  assert_eq!(buf[25], 0x00);
}

#[test]
fn envelope_priority_user_is_0x01() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 1, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[25], 0x01);
}

#[test]
fn envelope_manifest_none_encodes_as_zero_tag_after_serializer_id() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  assert_eq!(buf[26], 0x01);
  assert_eq!(&buf[27..35], &100_u64.to_be_bytes());
  assert_eq!(&buf[35..39], &5_u32.to_be_bytes());
  assert_eq!(buf[39], 0x00);
}

#[test]
fn envelope_sender_path_none_encodes_as_zero_tag() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  // After recipient:
  // [len(4)][version(1)][kind(1)][recipient_len(4)=0x00000002][recipient(2)][sender_tag] sender_tag
  // index = 4 + 1 + 1 + 4 + 2 = 12
  assert_eq!(buf[12], 0x00);
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
  buf[25] = 0x09;

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn envelope_decode_rejects_unknown_redelivery_flag() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 1, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf[26] = 0x02;

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
}

#[test]
fn envelope_decode_rejects_system_redelivery_flag_without_sequence_bytes() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(31);
  patch_frame_len(&mut buf);

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn envelope_decode_rejects_missing_serializer_id_after_redelivery_sequence() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf.truncate(35);
  patch_frame_len(&mut buf);

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::Truncated);
}

#[test]
fn envelope_decode_rejects_system_envelope_without_redelivery_sequence() {
  let pdu = test_envelope_pdu("/r".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  buf[26] = 0x00;
  let tail = buf.split_off(35);
  buf.truncate(27);
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
  buf[26] = 0x01;
  let tail = buf.split_off(27);
  buf.extend_from_slice(&10_u64.to_be_bytes());
  buf.unsplit(tail);
  patch_frame_len(&mut buf);

  let err = EnvelopeCodec::new().decode(&mut to_bytes(buf)).unwrap_err();

  assert_eq!(err, WireError::InvalidFormat);
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
  let kinds = [KIND_ENVELOPE, KIND_HANDSHAKE_REQ, KIND_HANDSHAKE_RSP, KIND_CONTROL, KIND_ACK];
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
  assert_eq!(WIRE_VERSION, WIRE_VERSION_2);
}

#[test]
fn string_hello_has_expected_bytes() {
  // Encode an envelope with a known-size recipient path so we can inspect the
  // length-prefixed string bytes for the spec's canonical example (`"hello"`).
  let pdu = test_envelope_pdu("hello".to_string(), None, 0, 0, 0, Bytes::new());
  let mut buf = BytesMut::new();
  EnvelopeCodec::new().encode(&pdu, &mut buf).unwrap();
  // layout: [len(4)][version(1)][kind(1)][recipient_len(4)][recipient_bytes...]
  // recipient_len starts at offset 6. For "hello" (5 bytes):
  assert_eq!(&buf[6..10], &[0x00, 0x00, 0x00, 0x05]);
  assert_eq!(&buf[10..15], b"hello");
}

#[test]
fn invalid_utf8_in_string_is_rejected() {
  // Manually construct an envelope frame whose recipient_path carries an
  // invalid UTF-8 byte sequence.
  let mut body = BytesMut::new();
  // recipient string: length 3, bytes 0xFF 0xFE 0xFD (invalid UTF-8)
  body.put_u32_len_prefix_invalid(3);
  body.extend_from_slice(&[0xFF, 0xFE, 0xFD]);
  // sender_path: None
  body.extend_from_slice(&[0x00]);
  // correlation_id: hi(u64) + lo(u32) = 96 bits
  body.extend_from_slice(&0u64.to_be_bytes());
  body.extend_from_slice(&0u32.to_be_bytes());
  // priority: u8
  body.extend_from_slice(&[0x00]);
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
