#![cfg(feature = "tokio-transport")]

use super::HeartbeatRsp;

#[test]
fn encode_decode_roundtrip_preserves_uid() {
  let original = HeartbeatRsp::new("node-a:9000", 42);
  let encoded = original.encode_frame();
  let decoded = HeartbeatRsp::decode_frame(&encoded, "node-a:9000").expect("decode_frame");
  assert_eq!(decoded, original);
}

#[test]
fn encode_decode_roundtrip_with_zero_uid() {
  let original = HeartbeatRsp::new("node-b:8080", 0);
  let encoded = original.encode_frame();
  let decoded = HeartbeatRsp::decode_frame(&encoded, "node-b:8080").expect("decode_frame");
  assert_eq!(decoded, original);
}

#[test]
fn encode_decode_roundtrip_with_max_uid() {
  let original = HeartbeatRsp::new("node-c:7070", u64::MAX);
  let encoded = original.encode_frame();
  let decoded = HeartbeatRsp::decode_frame(&encoded, "node-c:7070").expect("decode_frame");
  assert_eq!(decoded, original);
}

#[test]
fn decode_frame_rejects_short_payload() {
  let result = HeartbeatRsp::decode_frame(&[1, 0x23], "node-x");
  assert!(result.is_err());
}

#[test]
fn decode_frame_rejects_wrong_version() {
  // VERSION=1 のところ 0 を設定
  let mut encoded = HeartbeatRsp::new("node-a", 1).encode_frame();
  encoded[0] = 0;
  let result = HeartbeatRsp::decode_frame(&encoded, "node-a");
  assert!(result.is_err());
}

#[test]
fn decode_frame_rejects_wrong_frame_kind() {
  let mut encoded = HeartbeatRsp::new("node-a", 1).encode_frame();
  encoded[1] = 0xFF;
  let result = HeartbeatRsp::decode_frame(&encoded, "node-a");
  assert!(result.is_err());
}
