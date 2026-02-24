#![cfg(any(test, feature = "test-support"))]

use alloc::{boxed::Box, string::ToString};
use core::convert::TryFrom;

use fraktor_actor_rs::core::{
  actor::actor_path::{ActorPath, ActorPathParts, GuardianKind},
  event::stream::CorrelationId,
  serialization::{SerializedMessage, SerializerId},
};

use super::AckedDelivery;
use crate::core::{envelope::SystemMessageEnvelope, remote_node_id::RemoteNodeId};

fn sample_path() -> ActorPath {
  let mut parts = ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 25520)));
  parts = parts.with_guardian(GuardianKind::User);
  let mut path = ActorPath::from_parts(parts);
  path = path.child("user");
  path.child("svc")
}

fn sample_node(system: &str, host: &str, port: u16, uid: u64) -> RemoteNodeId {
  RemoteNodeId::new(system.to_string(), host.to_string(), Some(port), uid)
}

fn sample_system_message() -> AckedDelivery {
  let payload = SerializedMessage::new(SerializerId::try_from(88).expect("id"), None, b"sys".to_vec());
  AckedDelivery::SystemMessage(Box::new(SystemMessageEnvelope::new(
    sample_path(),
    sample_node("remote-system", "127.0.0.1", 25520, 9),
    None,
    payload,
    CorrelationId::from_u128(1),
    17,
    sample_node("local-system", "127.0.0.1", 2552, 2),
  )))
}

#[test]
fn round_trip_ack_and_nack_frames() {
  let ack = AckedDelivery::ack(11);
  let ack_decoded = AckedDelivery::decode_frame(&ack.encode_frame(), CorrelationId::nil()).expect("ack decode");
  assert!(ack_decoded.is_ack());
  assert_eq!(ack_decoded.sequence_no(), 11);

  let nack = AckedDelivery::nack(12);
  let nack_decoded = AckedDelivery::decode_frame(&nack.encode_frame(), CorrelationId::nil()).expect("nack decode");
  assert!(nack_decoded.is_nack());
  assert_eq!(nack_decoded.sequence_no(), 12);
}

#[test]
fn round_trip_system_message_frame() {
  let payload = sample_system_message();
  let decoded = AckedDelivery::decode_frame(&payload.encode_frame(), CorrelationId::from_u128(1)).expect("decode");
  match decoded {
    | AckedDelivery::SystemMessage(envelope) => {
      assert_eq!(envelope.sequence_no(), 17);
      assert_eq!(envelope.remote_node().system(), "remote-system");
      assert_eq!(envelope.ack_reply_to().system(), "local-system");
    },
    | other => panic!("unexpected payload: {other:?}"),
  }
}
