#![cfg(any(test, feature = "test-support"))]

use super::FlushAck;
use crate::core::control_message::ControlMessage;

#[test]
fn round_trip_flush_ack_frame() {
  let ack = FlushAck::new(3);
  let encoded = ack.encode_frame();
  let decoded = FlushAck::decode_frame(&encoded).expect("flush-ack decode");
  assert_eq!(decoded, ack);
  assert_eq!(decoded.expected_acks(), 3);
}

#[test]
fn flush_ack_reports_control_frame_kind() {
  let ack = FlushAck::new(0);
  assert_eq!(ack.frame_kind(), super::FLUSH_ACK_FRAME_KIND);
}
