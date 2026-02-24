#![cfg(any(test, feature = "test-support"))]

use alloc::string::ToString;
use core::convert::TryFrom;

use fraktor_actor_rs::core::{
  actor::actor_path::{ActorPath, ActorPathParts, GuardianKind},
  event::stream::CorrelationId,
  serialization::{SerializedMessage, SerializerId},
};

use super::SystemMessageEnvelope;
use crate::core::{envelope::RemotingEnvelope, remote_node_id::RemoteNodeId};

fn sample_recipient() -> ActorPath {
  let mut parts = ActorPathParts::with_authority("remote-system", Some(("127.0.0.1", 25520)));
  parts = parts.with_guardian(GuardianKind::User);
  let mut path = ActorPath::from_parts(parts);
  path = path.child("user");
  path.child("svc")
}

fn sample_node(system: &str, host: &str, port: u16, uid: u64) -> RemoteNodeId {
  RemoteNodeId::new(system.to_string(), host.to_string(), Some(port), uid)
}

fn sample_message() -> SerializedMessage {
  SerializedMessage::new(SerializerId::try_from(91).expect("serializer id"), None, b"payload".to_vec())
}

#[test]
fn round_trip_system_message_envelope_frame() {
  let envelope = SystemMessageEnvelope::new(
    sample_recipient(),
    sample_node("remote-system", "127.0.0.1", 25520, 11),
    None,
    sample_message(),
    CorrelationId::from_u128(10),
    7,
    sample_node("local-system", "127.0.0.1", 2552, 1),
  );

  let encoded = envelope.encode_frame();
  let decoded = SystemMessageEnvelope::decode_frame(&encoded, CorrelationId::from_u128(10)).expect("decode");
  assert_eq!(decoded.sequence_no(), 7);
  assert_eq!(decoded.remote_node().system(), "remote-system");
  assert_eq!(decoded.ack_reply_to().system(), "local-system");
  assert_eq!(decoded.serialized_message().bytes(), b"payload");
}

#[test]
fn converts_back_into_remoting_envelope_with_system_priority() {
  let system_message = SystemMessageEnvelope::new(
    sample_recipient(),
    sample_node("remote-system", "127.0.0.1", 25520, 11),
    None,
    sample_message(),
    CorrelationId::from_u128(44),
    3,
    sample_node("local-system", "127.0.0.1", 2552, 1),
  );
  let remoting = system_message.into_remoting_envelope();
  assert!(remoting.is_system());
  assert_eq!(remoting.correlation_id(), CorrelationId::from_u128(44));
}

#[test]
fn wraps_existing_remoting_envelope() {
  let remoting = RemotingEnvelope::new(
    sample_recipient(),
    sample_node("remote-system", "127.0.0.1", 25520, 11),
    None,
    sample_message(),
    CorrelationId::from_u128(55),
    crate::core::envelope::OutboundPriority::System,
  );
  let wrapped =
    SystemMessageEnvelope::from_remoting_envelope(remoting, 22, sample_node("local-system", "127.0.0.1", 2552, 1));
  assert_eq!(wrapped.sequence_no(), 22);
  assert_eq!(wrapped.correlation_id(), CorrelationId::from_u128(55));
}
