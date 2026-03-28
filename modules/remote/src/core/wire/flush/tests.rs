#![cfg(any(test, feature = "test-support"))]

use super::Flush;

#[test]
fn round_trip_flush_frame() {
  let flush = Flush::new();
  let encoded = flush.encode_frame();
  let decoded = Flush::decode_frame(&encoded).expect("flush decode");
  assert_eq!(decoded, flush);
}

#[test]
fn flush_reports_control_frame_kind() {
  let flush = Flush::new();
  assert_eq!(flush.frame_kind(), super::FLUSH_FRAME_KIND);
}
