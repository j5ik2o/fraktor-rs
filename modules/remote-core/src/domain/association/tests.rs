use alloc::{string::String, vec::Vec};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    actor_path::{ActorPath, ActorPathParser},
    messaging::AnyMessage,
  },
  event::stream::{CorrelationId, RemotingLifecycleEvent},
};

use crate::domain::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::{Association, AssociationEffect, AssociationState, OfferOutcome, QuarantineReason, SendQueue},
  envelope::{OutboundEnvelope, OutboundPriority},
  transport::{BackpressureSignal, TransportEndpoint},
};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn sample_local() -> UniqueAddress {
  UniqueAddress::new(Address::new("local-sys", "127.0.0.1", 2551), 1)
}

fn sample_remote_addr() -> Address {
  Address::new("remote-sys", "10.0.0.1", 2552)
}

fn sample_endpoint() -> TransportEndpoint {
  TransportEndpoint::new("remote-sys@10.0.0.1:2552")
}

fn sample_remote_node() -> RemoteNodeId {
  RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 0xdead_beef)
}

fn sample_path() -> ActorPath {
  ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse")
}

fn make_envelope(priority: OutboundPriority, payload: &str) -> OutboundEnvelope {
  OutboundEnvelope::new(
    sample_path(),
    None,
    AnyMessage::new(String::from(payload)),
    priority,
    sample_remote_node(),
    CorrelationId::nil(),
  )
}

fn new_association() -> Association {
  Association::new(sample_local(), sample_remote_addr())
}

// ---------------------------------------------------------------------------
// SendQueue behaviour
// ---------------------------------------------------------------------------

#[test]
fn send_queue_offer_always_accepts_in_phase_a() {
  let mut queue = SendQueue::new();
  let out = queue.offer(make_envelope(OutboundPriority::User, "x"));
  assert_eq!(out, OfferOutcome::Accepted);
  assert_eq!(queue.len(), 1);
}

#[test]
fn send_queue_drains_system_before_user() {
  let mut queue = SendQueue::new();
  let _ = queue.offer(make_envelope(OutboundPriority::User, "u1"));
  let _ = queue.offer(make_envelope(OutboundPriority::System, "s1"));
  let _ = queue.offer(make_envelope(OutboundPriority::User, "u2"));
  let _ = queue.offer(make_envelope(OutboundPriority::System, "s2"));

  // System first (s1, s2), then user (u1, u2).
  let first = queue.next_outbound().expect("first");
  assert!(matches!(first.priority(), OutboundPriority::System));
  let second = queue.next_outbound().expect("second");
  assert!(matches!(second.priority(), OutboundPriority::System));
  let third = queue.next_outbound().expect("third");
  assert!(matches!(third.priority(), OutboundPriority::User));
  let fourth = queue.next_outbound().expect("fourth");
  assert!(matches!(fourth.priority(), OutboundPriority::User));
  assert!(queue.next_outbound().is_none());
}

#[test]
fn send_queue_backpressure_pauses_user_lane_but_not_system() {
  let mut queue = SendQueue::new();
  let _ = queue.offer(make_envelope(OutboundPriority::User, "u1"));
  let _ = queue.offer(make_envelope(OutboundPriority::System, "s1"));
  queue.apply_backpressure(BackpressureSignal::Apply);
  assert!(queue.is_user_paused());

  let first = queue.next_outbound().expect("system before pause");
  assert!(matches!(first.priority(), OutboundPriority::System));
  // User is paused, so nothing comes out now.
  assert!(queue.next_outbound().is_none());
  assert_eq!(queue.len(), 1);

  queue.apply_backpressure(BackpressureSignal::Release);
  assert!(!queue.is_user_paused());
  let second = queue.next_outbound().expect("user after release");
  assert!(matches!(second.priority(), OutboundPriority::User));
}

#[test]
fn send_queue_with_capacity_still_grows_beyond_hint() {
  let mut queue = SendQueue::with_capacity(1, 1);
  for i in 0..5 {
    let _ = queue.offer(make_envelope(OutboundPriority::User, &alloc::format!("u{i}")));
  }
  // Unbounded in Phase A — all 5 are retained.
  assert_eq!(queue.len(), 5);
}

// ---------------------------------------------------------------------------
// Association state machine
// ---------------------------------------------------------------------------

#[test]
fn idle_to_handshaking_to_active_happy_path() {
  let mut a = new_association();
  assert!(a.state().is_idle());

  let effects = a.associate(sample_endpoint(), 100);
  assert!(matches!(a.state(), AssociationState::Handshaking { .. }));
  assert!(matches!(effects.as_slice(), [AssociationEffect::StartHandshake { .. }]));

  let effects = a.handshake_accepted(sample_remote_node(), 200);
  assert!(a.state().is_active());
  // First effect is the Connected lifecycle publish.
  assert!(matches!(
    effects.first(),
    Some(AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Connected { .. }))
  ));
}

#[test]
fn handshaking_timeout_transitions_to_gated_with_lifecycle() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let effects = a.handshake_timed_out(100, Some(500));
  assert!(a.state().is_gated());
  // Publish Gated lifecycle (deferred queue empty → no DiscardEnvelopes).
  assert_eq!(effects.len(), 1);
  assert!(matches!(effects.first(), Some(AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Gated { .. }))));
}

#[test]
fn handshaking_timeout_with_deferred_envelopes_emits_discard() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  // Queue two deferred envelopes during handshake.
  let _ = a.enqueue(make_envelope(OutboundPriority::User, "u1"));
  let _ = a.enqueue(make_envelope(OutboundPriority::User, "u2"));
  assert_eq!(a.deferred_len(), 2);

  let effects = a.handshake_timed_out(100, None);
  assert!(effects.iter().any(|e| matches!(e, AssociationEffect::DiscardEnvelopes { .. })));
  assert_eq!(a.deferred_len(), 0);
}

#[test]
fn active_to_quarantined_publishes_and_discards_pending() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let _ = a.handshake_accepted(sample_remote_node(), 10);

  // Put an envelope into the send queue while Active.
  let _ = a.enqueue(make_envelope(OutboundPriority::User, "u1"));
  assert!(!a.send_queue().is_empty());

  let effects = a.quarantine(QuarantineReason::new("fatal"), 20);
  assert!(a.state().is_quarantined());
  assert!(a.send_queue().is_empty());
  assert!(
    effects
      .iter()
      .any(|e| matches!(e, AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Quarantined { .. })))
  );
  assert!(effects.iter().any(|e| matches!(e, AssociationEffect::DiscardEnvelopes { .. })));
}

#[test]
fn recover_some_endpoint_from_gated_starts_handshake() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let _ = a.handshake_timed_out(10, None); // Gated
  assert!(a.state().is_gated());

  let effects = a.recover(Some(sample_endpoint()), 30);
  assert!(matches!(a.state(), AssociationState::Handshaking { started_at: 30, .. }));
  assert!(matches!(effects.as_slice(), [AssociationEffect::StartHandshake { .. }]));
}

#[test]
fn recover_some_endpoint_from_quarantined_starts_handshake() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let _ = a.quarantine(QuarantineReason::new("boom"), 10);
  assert!(a.state().is_quarantined());

  let effects = a.recover(Some(sample_endpoint()), 50);
  assert!(matches!(a.state(), AssociationState::Handshaking { .. }));
  assert_eq!(effects.len(), 1);
  assert!(matches!(effects[0], AssociationEffect::StartHandshake { .. }));
}

#[test]
fn recover_none_from_gated_returns_to_idle() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let _ = a.handshake_timed_out(10, None); // Gated

  let effects = a.recover(None, 20);
  assert!(a.state().is_idle());
  assert!(effects.is_empty());
}

#[test]
fn recover_from_active_is_no_op() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let _ = a.handshake_accepted(sample_remote_node(), 5);
  assert!(a.state().is_active());

  let effects = a.recover(Some(sample_endpoint()), 10);
  assert!(a.state().is_active(), "Active state should be untouched by recover");
  assert!(effects.is_empty());
}

#[test]
fn recover_from_idle_is_no_op() {
  let mut a = new_association();
  assert!(a.state().is_idle());
  let e1 = a.recover(Some(sample_endpoint()), 10);
  let e2 = a.recover(None, 10);
  assert!(a.state().is_idle());
  assert!(e1.is_empty());
  assert!(e2.is_empty());
}

#[test]
fn recover_from_handshaking_is_no_op() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let effects = a.recover(Some(sample_endpoint()), 10);
  assert!(matches!(a.state(), AssociationState::Handshaking { .. }));
  assert!(effects.is_empty());
}

// ---------------------------------------------------------------------------
// enqueue semantics per state
// ---------------------------------------------------------------------------

#[test]
fn enqueue_in_active_pushes_into_send_queue() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let _ = a.handshake_accepted(sample_remote_node(), 10);

  let effects = a.enqueue(make_envelope(OutboundPriority::User, "u1"));
  assert!(effects.is_empty());
  assert_eq!(a.send_queue().len(), 1);
  assert_eq!(a.deferred_len(), 0);
}

#[test]
fn enqueue_in_handshaking_pushes_into_deferred() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);

  let effects = a.enqueue(make_envelope(OutboundPriority::User, "u1"));
  assert!(effects.is_empty());
  assert_eq!(a.deferred_len(), 1);
  assert!(a.send_queue().is_empty());
}

#[test]
fn enqueue_in_gated_pushes_into_deferred() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let _ = a.handshake_accepted(sample_remote_node(), 10);
  let _ = a.gate(Some(100), 20);
  assert!(a.state().is_gated());

  let effects = a.enqueue(make_envelope(OutboundPriority::User, "u1"));
  assert!(effects.is_empty());
  assert_eq!(a.deferred_len(), 1);
}

#[test]
fn enqueue_in_idle_pushes_into_deferred() {
  let mut a = new_association();
  let effects = a.enqueue(make_envelope(OutboundPriority::User, "u1"));
  assert!(effects.is_empty());
  assert_eq!(a.deferred_len(), 1);
}

#[test]
fn enqueue_in_quarantined_emits_discard_effect() {
  let mut a = new_association();
  let _ = a.quarantine(QuarantineReason::new("nope"), 0);
  assert!(a.state().is_quarantined());

  let effects = a.enqueue(make_envelope(OutboundPriority::User, "u1"));
  let discards: Vec<_> = effects.iter().filter(|e| matches!(e, AssociationEffect::DiscardEnvelopes { .. })).collect();
  assert_eq!(discards.len(), 1);
  // Nothing should have been deferred / enqueued.
  assert_eq!(a.deferred_len(), 0);
  assert!(a.send_queue().is_empty());
}

#[test]
fn deferred_envelopes_flush_on_handshake_accepted() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let _ = a.enqueue(make_envelope(OutboundPriority::User, "u1"));
  let _ = a.enqueue(make_envelope(OutboundPriority::User, "u2"));

  let effects = a.handshake_accepted(sample_remote_node(), 10);
  // Expect a SendEnvelopes effect flushing the deferred queue.
  let send =
    effects.iter().find(|e| matches!(e, AssociationEffect::SendEnvelopes { .. })).expect("SendEnvelopes effect");
  if let AssociationEffect::SendEnvelopes { envelopes } = send {
    assert_eq!(envelopes.len(), 2);
  }
  assert_eq!(a.deferred_len(), 0);
}

// ---------------------------------------------------------------------------
// next_outbound / apply_backpressure pass-through
// ---------------------------------------------------------------------------

#[test]
fn next_outbound_returns_system_then_user_through_association() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let _ = a.handshake_accepted(sample_remote_node(), 10);
  let _ = a.enqueue(make_envelope(OutboundPriority::User, "u1"));
  let _ = a.enqueue(make_envelope(OutboundPriority::System, "s1"));

  let first = a.next_outbound().expect("first");
  assert!(matches!(first.priority(), OutboundPriority::System));
  let second = a.next_outbound().expect("second");
  assert!(matches!(second.priority(), OutboundPriority::User));
  assert!(a.next_outbound().is_none());
}

#[test]
fn apply_backpressure_propagates_to_send_queue() {
  let mut a = new_association();
  let _ = a.associate(sample_endpoint(), 0);
  let _ = a.handshake_accepted(sample_remote_node(), 10);
  let _ = a.enqueue(make_envelope(OutboundPriority::User, "u1"));

  a.apply_backpressure(BackpressureSignal::Apply);
  assert!(a.send_queue().is_user_paused());
  assert!(a.next_outbound().is_none(), "user lane should be paused");

  a.apply_backpressure(BackpressureSignal::Release);
  assert!(!a.send_queue().is_user_paused());
  let env = a.next_outbound().expect("released");
  assert!(matches!(env.priority(), OutboundPriority::User));
}
