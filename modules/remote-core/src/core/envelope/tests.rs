use alloc::string::String;

use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_path::{ActorPath, ActorPathParser},
    messaging::AnyMessage,
  },
  event::stream::CorrelationId,
};

use crate::core::{
  address::RemoteNodeId,
  envelope::{InboundEnvelope, OutboundEnvelope, OutboundPriority},
};

fn sample_path(uri: &str) -> ActorPath {
  ActorPathParser::parse(uri).expect("parse actor path")
}

fn sample_remote_node() -> RemoteNodeId {
  RemoteNodeId::new("sys", "host", Some(2552), 7)
}

#[test]
fn priority_system_maps_to_wire_zero() {
  assert_eq!(OutboundPriority::System.to_wire(), 0);
  assert_eq!(OutboundPriority::from_wire(0), Some(OutboundPriority::System));
  assert!(OutboundPriority::System.is_system());
}

#[test]
fn priority_user_maps_to_wire_one() {
  assert_eq!(OutboundPriority::User.to_wire(), 1);
  assert_eq!(OutboundPriority::from_wire(1), Some(OutboundPriority::User));
  assert!(!OutboundPriority::User.is_system());
}

#[test]
fn priority_from_wire_rejects_unknown_value() {
  assert_eq!(OutboundPriority::from_wire(2), None);
  assert_eq!(OutboundPriority::from_wire(0xFF), None);
}

#[test]
fn outbound_envelope_exposes_all_accessors() {
  let recipient = sample_path("fraktor.tcp://sys@host:2552/user/r");
  let sender = Some(sample_path("fraktor.tcp://sys@host:2552/user/s"));
  let message = AnyMessage::new(String::from("hello"));
  let remote = sample_remote_node();
  let corr = CorrelationId::new(1, 2);

  let env =
    OutboundEnvelope::new(recipient.clone(), sender.clone(), message, OutboundPriority::User, remote.clone(), corr);

  assert_eq!(env.recipient(), &recipient);
  assert_eq!(env.sender(), sender.as_ref());
  assert_eq!(env.priority(), OutboundPriority::User);
  assert_eq!(env.remote_node(), &remote);
  assert_eq!(env.correlation_id(), corr);
  // Just check we can borrow the payload — AnyMessage itself is opaque here.
  let _payload: &AnyMessage = env.message();
}

#[test]
fn outbound_envelope_none_sender_is_none() {
  let env = OutboundEnvelope::new(
    sample_path("fraktor.tcp://sys@host:2552/user/r"),
    None,
    AnyMessage::new(()),
    OutboundPriority::System,
    sample_remote_node(),
    CorrelationId::nil(),
  );
  assert!(env.sender().is_none());
  assert!(env.priority().is_system());
}

#[test]
fn outbound_envelope_into_parts_returns_all_fields() {
  let recipient = sample_path("fraktor.tcp://sys@host:2552/user/r");
  let sender = sample_path("fraktor.tcp://sys@host:2552/user/s");
  let remote = sample_remote_node();
  let corr = CorrelationId::new(9, 8);

  let env = OutboundEnvelope::new(
    recipient.clone(),
    Some(sender.clone()),
    AnyMessage::new(42u32),
    OutboundPriority::User,
    remote.clone(),
    corr,
  );
  let (r, s, _msg, pr, rn, c) = env.into_parts();
  assert_eq!(r, recipient);
  assert_eq!(s, Some(sender));
  assert_eq!(pr, OutboundPriority::User);
  assert_eq!(rn, remote);
  assert_eq!(c, corr);
}

#[test]
fn inbound_envelope_exposes_all_accessors() {
  let recipient = sample_path("fraktor.tcp://sys@host:2552/user/r");
  let sender = Some(sample_path("fraktor.tcp://sys@host:2552/user/s"));
  let remote = sample_remote_node();
  let corr = CorrelationId::new(7, 1);

  let env = InboundEnvelope::new(
    recipient.clone(),
    remote.clone(),
    AnyMessage::new(String::from("payload")),
    sender.clone(),
    corr,
    OutboundPriority::System,
  );

  assert_eq!(env.recipient(), &recipient);
  assert_eq!(env.remote_node(), &remote);
  assert_eq!(env.sender(), sender.as_ref());
  assert_eq!(env.correlation_id(), corr);
  assert_eq!(env.priority(), OutboundPriority::System);
}

#[test]
fn inbound_envelope_into_parts_returns_all_fields() {
  let recipient = sample_path("fraktor.tcp://sys@host:2552/user/r");
  let remote = sample_remote_node();
  let corr = CorrelationId::new(3, 4);

  let env =
    InboundEnvelope::new(recipient.clone(), remote.clone(), AnyMessage::new(()), None, corr, OutboundPriority::User);
  let (r, rn, _msg, s, c, pr) = env.into_parts();
  assert_eq!(r, recipient);
  assert_eq!(rn, remote);
  assert!(s.is_none());
  assert_eq!(c, corr);
  assert_eq!(pr, OutboundPriority::User);
}
