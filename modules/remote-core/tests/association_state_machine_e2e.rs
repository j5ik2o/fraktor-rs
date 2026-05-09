//! End-to-end integration test for the
//! [`fraktor_remote_core_rs::core::association::Association`] state machine.
//!
//! Exercises the full happy-path lifecycle of an `Association` from `Idle`
//! through `Handshaking` → `Active` → `Quarantined` while verifying the
//! emitted side-effects are consistent with the design specification.

use alloc::{string::String, vec::Vec};

extern crate alloc;

use fraktor_actor_core_rs::{
  actor::{actor_path::ActorPathParser, messaging::AnyMessage},
  event::stream::CorrelationId,
};
use fraktor_remote_core_rs::core::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::{Association, AssociationEffect, AssociationState, QuarantineReason},
  envelope::{OutboundEnvelope, OutboundPriority},
  instrument::RemotingFlightRecorder,
  transport::{BackpressureSignal, TransportEndpoint},
  wire::HandshakeRsp,
};

fn local_address() -> UniqueAddress {
  UniqueAddress::new(Address::new("local-sys", "127.0.0.1", 2551), 1)
}

fn remote_address() -> Address {
  Address::new("remote-sys", "10.0.0.1", 2552)
}

fn remote_node() -> RemoteNodeId {
  RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 7)
}

fn remote_unique_address() -> UniqueAddress {
  UniqueAddress::new(remote_address(), 7)
}

fn user_envelope(payload: &str) -> OutboundEnvelope {
  let path =
    ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/svc").expect("parse remote actor path");
  OutboundEnvelope::new(
    path,
    None,
    AnyMessage::new(String::from(payload)),
    OutboundPriority::User,
    remote_node(),
    CorrelationId::nil(),
  )
}

fn system_envelope(payload: &str) -> OutboundEnvelope {
  let path =
    ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/svc").expect("parse remote actor path");
  OutboundEnvelope::new(
    path,
    None,
    AnyMessage::new(String::from(payload)),
    OutboundPriority::System,
    remote_node(),
    CorrelationId::nil(),
  )
}

fn enqueue(association: &mut Association, envelope: OutboundEnvelope, now_ms: u64) -> Vec<AssociationEffect> {
  {
    let mut instrument = RemotingFlightRecorder::new(0);
    association.enqueue(envelope, now_ms, &mut instrument)
  }
}

#[test]
fn full_lifecycle_associate_handshake_send_quarantine_recover() {
  let mut association = Association::new(local_address(), remote_address());
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let mut instrument = RemotingFlightRecorder::new(0);

  // 1. Idle → Handshaking → Active
  let effects = association.associate(endpoint.clone(), 100, &mut instrument);
  assert!(matches!(effects.as_slice(), [AssociationEffect::StartHandshake { .. }]));

  // While Handshaking, enqueue should defer messages.
  let _ = enqueue(&mut association, user_envelope("u1"), 100);
  let _ = enqueue(&mut association, user_envelope("u2"), 100);
  assert_eq!(association.deferred_len(), 2);

  // Complete the handshake → Active.
  let response = HandshakeRsp::new(remote_unique_address());
  let effects = association
    .accept_handshake_response(&response, 200, &mut instrument)
    .expect("matching handshake response should be accepted");
  let send_effect = effects.iter().find(|effect| matches!(effect, AssociationEffect::SendEnvelopes { .. }));
  assert!(send_effect.is_some(), "deferred queue should flush as SendEnvelopes");
  if let Some(AssociationEffect::SendEnvelopes { envelopes }) = send_effect {
    assert_eq!(envelopes.len(), 2);
  }
  assert!(association.state().is_active());

  // 2. While Active, enqueue routes through the send queue.
  let _ = enqueue(&mut association, system_envelope("s1"), 200);
  let _ = enqueue(&mut association, user_envelope("u3"), 200);
  assert!(!association.send_queue().is_empty());

  // System priority drains first.
  let next = association.next_outbound(200, &mut instrument).expect("system message");
  assert!(matches!(next.priority(), OutboundPriority::System));
  let next = association.next_outbound(200, &mut instrument).expect("user message");
  assert!(matches!(next.priority(), OutboundPriority::User));

  // 3. Backpressure pauses the user lane.
  let _ = enqueue(&mut association, user_envelope("u4"), 300);
  let _ = enqueue(&mut association, system_envelope("s2"), 300);
  association.apply_backpressure(BackpressureSignal::Apply, CorrelationId::nil(), 300, &mut instrument);
  let next = association.next_outbound(300, &mut instrument).expect("system bypasses backpressure");
  assert!(matches!(next.priority(), OutboundPriority::System));
  assert!(
    association.next_outbound(300, &mut instrument).is_none(),
    "user lane must remain paused while backpressure is applied"
  );
  association.apply_backpressure(BackpressureSignal::Release, CorrelationId::nil(), 300, &mut instrument);
  let next = association.next_outbound(300, &mut instrument).expect("user resumes after release");
  assert!(matches!(next.priority(), OutboundPriority::User));

  // 4. Quarantine discards every pending envelope.
  let _ = enqueue(&mut association, user_envelope("u5"), 400);
  let effects = association.quarantine(QuarantineReason::new("e2e test"), 1_000, &mut instrument);
  assert!(association.state().is_quarantined());
  assert!(
    effects.iter().any(|e| matches!(e, AssociationEffect::DiscardEnvelopes { .. })),
    "quarantine must emit DiscardEnvelopes for any pending traffic"
  );

  // 5. recover(Some(endpoint)) revives the association into Handshaking.
  let effects = association.recover(Some(endpoint.clone()), 2_000, &mut instrument);
  assert!(matches!(effects.as_slice(), [AssociationEffect::StartHandshake { .. }]));
  assert!(matches!(association.state(), AssociationState::Handshaking { .. }));
}

#[test]
fn enqueue_in_quarantined_state_immediately_emits_discard() {
  let mut association = Association::new(local_address(), remote_address());
  let mut instrument = RemotingFlightRecorder::new(0);
  let _ = association.quarantine(QuarantineReason::new("immediate"), 0, &mut instrument);
  let effects = enqueue(&mut association, user_envelope("u1"), 0);
  let discards: Vec<_> =
    effects.iter().filter(|effect| matches!(effect, AssociationEffect::DiscardEnvelopes { .. })).collect();
  assert_eq!(discards.len(), 1);
  assert!(association.send_queue().is_empty());
  assert_eq!(association.deferred_len(), 0);
}
