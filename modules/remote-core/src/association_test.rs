use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_actor_core_kernel_rs::{
  actor::{
    actor_path::{ActorPath, ActorPathParser},
    messaging::AnyMessage,
  },
  event::stream::{CorrelationId, RemotingLifecycleEvent},
};

use crate::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::{
    Association, AssociationEffect, AssociationState, HandshakeRejectedState, HandshakeValidationError, OfferOutcome,
    QuarantineReason, SendQueue,
  },
  config::{LargeMessageDestinationPattern, LargeMessageDestinations, RemoteConfig},
  envelope::{OutboundEnvelope, OutboundPriority},
  instrument::{FlightRecorderEvent, HandshakePhase, NoopInstrument, RemotingFlightRecorder},
  transport::{BackpressureSignal, TransportEndpoint},
  wire::{AckPdu, HandshakeReq, HandshakeRsp},
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

fn sample_remote_unique() -> UniqueAddress {
  UniqueAddress::new(sample_remote_addr(), 0xdead_beef)
}

fn other_remote_unique() -> UniqueAddress {
  UniqueAddress::new(Address::new("other-sys", "10.0.0.9", 2559), 0xbeef)
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

fn make_envelope_to(path: &str, priority: OutboundPriority, payload: &str) -> OutboundEnvelope {
  let recipient =
    ActorPathParser::parse(&alloc::format!("fraktor.tcp://remote-sys@10.0.0.1:2552{path}")).expect("parse");
  OutboundEnvelope::new(
    recipient,
    None,
    AnyMessage::new(String::from(payload)),
    priority,
    sample_remote_node(),
    CorrelationId::nil(),
  )
}

fn payload_string(envelope: &OutboundEnvelope) -> &str {
  envelope.message().downcast_ref::<String>().expect("test payload should be String")
}

fn large_message_destinations(pattern: &str) -> LargeMessageDestinations {
  LargeMessageDestinations::new().with_pattern(LargeMessageDestinationPattern::new(pattern))
}

fn assert_offer_accepted(outcome: &OfferOutcome) {
  assert!(matches!(outcome, OfferOutcome::Accepted), "expected Accepted, got {outcome:?}");
}

fn new_association() -> Association {
  Association::new(sample_local(), sample_remote_addr())
}

fn enqueue(association: &mut Association, envelope: OutboundEnvelope, now_ms: u64) -> Vec<AssociationEffect> {
  association.enqueue(envelope, now_ms, &mut NoopInstrument)
}

fn associate_idle(association: &mut Association, now_ms: u64) {
  let effects = association.associate(sample_endpoint(), now_ms, &mut NoopInstrument);
  assert!(!effects.is_empty(), "associate should emit StartHandshake");
}

fn time_out_handshake(association: &mut Association, now_ms: u64, resume_at_ms: Option<u64>) {
  let effects = association.handshake_timed_out(now_ms, resume_at_ms, &mut NoopInstrument);
  assert!(!effects.is_empty(), "handshake timeout should emit lifecycle effects");
}

fn active_association_from_config(config: &RemoteConfig) -> Association {
  let mut association = Association::from_config(sample_local(), sample_remote_addr(), config);
  associate_idle(&mut association, 0);
  let response = HandshakeRsp::new(sample_remote_unique());
  let effects = association
    .accept_handshake_response(&response, 10, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  assert!(!effects.is_empty(), "handshake response should emit lifecycle effects");
  assert!(association.state().is_active());
  association
}

// ---------------------------------------------------------------------------
// SendQueue behaviour
// ---------------------------------------------------------------------------

#[test]
fn send_queue_offer_accepts_when_lane_has_capacity() {
  let mut queue = SendQueue::new();
  let out = queue.offer(make_envelope(OutboundPriority::User, "x"));
  assert_offer_accepted(&out);
  assert_eq!(queue.len(), 1);
}

#[test]
fn send_queue_drains_system_before_user() {
  let mut queue = SendQueue::new();
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::User, "u1")));
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::System, "s1")));
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::User, "u2")));
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::System, "s2")));

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
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::User, "u1")));
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::System, "s1")));
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
fn send_queue_rejects_user_envelope_when_user_lane_is_full() {
  let mut queue = SendQueue::with_capacity(10, 2);
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::User, "u1")));
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::User, "u2")));

  let outcome = queue.offer(make_envelope(OutboundPriority::User, "u3"));

  match outcome {
    | OfferOutcome::QueueFull { envelope } => {
      assert!(matches!(envelope.priority(), OutboundPriority::User));
    },
    | other => panic!("expected QueueFull for full user lane, got {other:?}"),
  }
  assert_eq!(queue.len(), 2);
}

#[test]
fn send_queue_rejects_system_envelope_when_system_lane_is_full() {
  let mut queue = SendQueue::with_capacity(1, 10);
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::System, "s1")));

  let outcome = queue.offer(make_envelope(OutboundPriority::System, "s2"));

  match outcome {
    | OfferOutcome::QueueFull { envelope } => {
      assert!(matches!(envelope.priority(), OutboundPriority::System));
    },
    | other => panic!("expected QueueFull for full system lane, got {other:?}"),
  }
  assert_eq!(queue.len(), 1);
}

#[test]
fn send_queue_with_limits_enforces_bounds_without_requiring_preallocation() {
  let mut queue = SendQueue::with_limits(1, 1);
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::System, "s1")));
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::User, "u1")));

  assert!(matches!(queue.offer(make_envelope(OutboundPriority::System, "s2")), OfferOutcome::QueueFull { .. }));
  assert!(matches!(queue.offer(make_envelope(OutboundPriority::User, "u2")), OfferOutcome::QueueFull { .. }));
}

#[test]
fn send_queue_drains_system_then_user_then_large_message() {
  let mut queue = SendQueue::with_lane_limits(10, 10, 10);
  assert_offer_accepted(&queue.offer_large_message(make_envelope(OutboundPriority::User, "large")));
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::User, "user")));
  assert_offer_accepted(&queue.offer(make_envelope(OutboundPriority::System, "system")));

  let first = queue.next_outbound().expect("system first");
  let second = queue.next_outbound().expect("user second");
  let third = queue.next_outbound().expect("large third");

  assert_eq!(payload_string(&first), "system");
  assert_eq!(payload_string(&second), "user");
  assert_eq!(payload_string(&third), "large");
  assert!(queue.next_outbound().is_none());
}

#[test]
fn send_queue_backpressure_pauses_large_message_lane_with_user_lane() {
  let mut queue = SendQueue::with_lane_limits(10, 10, 10);
  assert_offer_accepted(&queue.offer_large_message(make_envelope(OutboundPriority::User, "large")));
  queue.apply_backpressure(BackpressureSignal::Apply);

  assert!(queue.next_outbound().is_none());

  queue.apply_backpressure(BackpressureSignal::Release);
  let envelope = queue.next_outbound().expect("large message after release");
  assert_eq!(payload_string(&envelope), "large");
}

#[test]
fn send_queue_rejects_large_message_envelope_when_large_message_lane_is_full() {
  let mut queue = SendQueue::with_lane_limits(10, 10, 1);
  assert_offer_accepted(&queue.offer_large_message(make_envelope(OutboundPriority::User, "large-1")));

  let outcome = queue.offer_large_message(make_envelope(OutboundPriority::User, "large-2"));

  match outcome {
    | OfferOutcome::QueueFull { envelope } => {
      assert_eq!(payload_string(&envelope), "large-2");
    },
    | other => panic!("expected QueueFull for full large-message lane, got {other:?}"),
  }
}

#[test]
fn send_queue_with_capacity_rejects_zero_system_capacity() {
  let result = std::panic::catch_unwind(|| SendQueue::with_capacity(0, 1));

  assert!(result.is_err(), "system lane capacity must reject zero");
}

#[test]
fn send_queue_with_capacity_rejects_zero_user_capacity() {
  let result = std::panic::catch_unwind(|| SendQueue::with_capacity(1, 0));

  assert!(result.is_err(), "user lane capacity must reject zero");
}

#[test]
fn send_queue_with_limits_rejects_zero_system_limit() {
  let result = std::panic::catch_unwind(|| SendQueue::with_limits(0, 1));

  assert!(result.is_err(), "system lane limit must reject zero");
}

#[test]
fn send_queue_with_limits_rejects_zero_user_limit() {
  let result = std::panic::catch_unwind(|| SendQueue::with_limits(1, 0));

  assert!(result.is_err(), "user lane limit must reject zero");
}

#[test]
fn send_queue_with_lane_limits_rejects_zero_large_message_limit() {
  let result = std::panic::catch_unwind(|| SendQueue::with_lane_limits(1, 1, 0));

  assert!(result.is_err(), "large-message lane limit must reject zero");
}

// ---------------------------------------------------------------------------
// Association state machine
// ---------------------------------------------------------------------------

#[test]
fn idle_to_handshaking_to_active_happy_path() {
  let mut a = new_association();
  assert!(a.state().is_idle());

  let effects = a.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(matches!(a.state(), AssociationState::Handshaking { .. }));
  assert!(matches!(effects.as_slice(), [AssociationEffect::StartHandshake { .. }]));

  let response = HandshakeRsp::new(sample_remote_unique());
  let effects = a
    .accept_handshake_response(&response, 200, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  assert!(a.state().is_active());
  // First effect is the Connected lifecycle publish.
  assert!(matches!(
    effects.first(),
    Some(AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Connected { .. }))
  ));
}

#[test]
fn associate_start_handshake_carries_timeout_and_generation() {
  let config = RemoteConfig::new("localhost").with_handshake_timeout(Duration::from_millis(250));
  let mut association = Association::from_config(sample_local(), sample_remote_addr(), &config);

  let effects = association.associate(sample_endpoint(), 100, &mut NoopInstrument);

  assert_eq!(association.handshake_generation(), 1);
  assert!(matches!(
    effects.as_slice(),
    [AssociationEffect::StartHandshake {
      authority,
      timeout,
      generation
    }] if authority == &sample_endpoint()
      && *timeout == Duration::from_millis(250)
      && *generation == 1
  ));
}

#[test]
fn accept_handshake_request_transitions_to_active_when_from_and_to_match() {
  let mut a = new_association();
  let started = a.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let request = HandshakeReq::new(sample_remote_unique(), sample_local().address().clone());

  let effects = a
    .accept_handshake_request(&request, 200, &mut NoopInstrument)
    .expect("matching handshake request should be accepted");

  assert!(matches!(
    a.state(),
    AssociationState::Active {
      remote_node,
      established_at: 200,
      ..
    } if remote_node.system() == "remote-sys" && remote_node.uid() == 0xdead_beef
  ));
  assert!(matches!(
    effects.first(),
    Some(AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Connected { .. }))
  ));
}

#[test]
fn accept_handshake_request_rejects_unexpected_destination_without_state_change() {
  let mut a = new_association();
  let started = a.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let request = HandshakeReq::new(sample_remote_unique(), Address::new("other-local", "127.0.0.2", 2551));

  let result = a.accept_handshake_request(&request, 200, &mut NoopInstrument);

  assert!(result.is_err(), "request addressed to a different local address must be rejected");
  assert!(matches!(a.state(), AssociationState::Handshaking { started_at: 100, .. }));
}

#[test]
fn accept_handshake_request_rejects_unexpected_remote_without_state_change() {
  let mut a = new_association();
  let started = a.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let request = HandshakeReq::new(other_remote_unique(), sample_local().address().clone());

  let result = a.accept_handshake_request(&request, 200, &mut NoopInstrument);

  assert!(result.is_err(), "request from a different remote address must be rejected");
  assert!(matches!(a.state(), AssociationState::Handshaking { started_at: 100, .. }));
}

// `Idle` / `Gated` / `Quarantined` の各状態でハンドシェイクを受理してしまうと、
// inbound dispatcher が Ok を見て HandshakeRsp を送り返し、リモートはハンドシェイク
// 成立と認識する一方でローカルは到達不能のまま、という非対称なプロトコル状態が
// 生まれる。これらの状態では accept_handshake_request / accept_handshake_response が
// `RejectedInState` を返し、状態遷移しないことを回帰検証する。

#[test]
fn accept_handshake_request_rejects_idle_state_without_state_change() {
  let mut a = new_association();
  assert!(matches!(a.state(), AssociationState::Idle));
  let request = HandshakeReq::new(sample_remote_unique(), sample_local().address().clone());

  let result = a.accept_handshake_request(&request, 200, &mut NoopInstrument);

  assert!(matches!(result, Err(HandshakeValidationError::RejectedInState { state: HandshakeRejectedState::Idle })));
  assert!(matches!(a.state(), AssociationState::Idle));
}

#[test]
fn accept_handshake_request_rejects_gated_state_without_state_change() {
  let mut a = new_association();
  associate_idle(&mut a, 100);
  // gate() は Active からしか遷移しないため、Handshaking → Gated は
  // handshake_timed_out() 経由で作る。
  time_out_handshake(&mut a, 100, Some(500));
  assert!(a.state().is_gated());
  let request = HandshakeReq::new(sample_remote_unique(), sample_local().address().clone());

  let result = a.accept_handshake_request(&request, 200, &mut NoopInstrument);

  assert!(matches!(result, Err(HandshakeValidationError::RejectedInState { state: HandshakeRejectedState::Gated })));
  assert!(a.state().is_gated());
}

#[test]
fn accept_handshake_request_rejects_quarantined_state_without_state_change() {
  let mut a = new_association();
  associate_idle(&mut a, 100);
  let _ = a.quarantine(QuarantineReason::new("handshake-timeout"), 100, &mut NoopInstrument);
  assert!(a.state().is_quarantined());
  let request = HandshakeReq::new(sample_remote_unique(), sample_local().address().clone());

  let result = a.accept_handshake_request(&request, 200, &mut NoopInstrument);

  assert!(matches!(
    result,
    Err(HandshakeValidationError::RejectedInState { state: HandshakeRejectedState::Quarantined })
  ));
  assert!(a.state().is_quarantined());
}

#[test]
fn accept_handshake_response_rejects_idle_state_without_state_change() {
  let mut a = new_association();
  assert!(matches!(a.state(), AssociationState::Idle));
  let response = HandshakeRsp::new(sample_remote_unique());

  let result = a.accept_handshake_response(&response, 200, &mut NoopInstrument);

  assert!(matches!(result, Err(HandshakeValidationError::RejectedInState { state: HandshakeRejectedState::Idle })));
  assert!(matches!(a.state(), AssociationState::Idle));
}

#[test]
fn accept_handshake_response_rejects_gated_state_without_state_change() {
  let mut a = new_association();
  associate_idle(&mut a, 100);
  time_out_handshake(&mut a, 100, Some(500));
  let response = HandshakeRsp::new(sample_remote_unique());

  let result = a.accept_handshake_response(&response, 200, &mut NoopInstrument);

  assert!(matches!(result, Err(HandshakeValidationError::RejectedInState { state: HandshakeRejectedState::Gated })));
  assert!(a.state().is_gated());
}

#[test]
fn accept_handshake_response_rejects_quarantined_state_without_state_change() {
  let mut a = new_association();
  associate_idle(&mut a, 100);
  let _ = a.quarantine(QuarantineReason::new("handshake-timeout"), 100, &mut NoopInstrument);
  let response = HandshakeRsp::new(sample_remote_unique());

  let result = a.accept_handshake_response(&response, 200, &mut NoopInstrument);

  assert!(matches!(
    result,
    Err(HandshakeValidationError::RejectedInState { state: HandshakeRejectedState::Quarantined })
  ));
  assert!(a.state().is_quarantined());
}

#[test]
fn accept_handshake_response_transitions_to_active_when_origin_matches_remote() {
  let mut a = new_association();
  let started = a.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let response = HandshakeRsp::new(sample_remote_unique());

  let effects = a
    .accept_handshake_response(&response, 200, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");

  assert!(matches!(
    a.state(),
    AssociationState::Active {
      remote_node,
      established_at: 200,
      ..
    } if remote_node.system() == "remote-sys" && remote_node.uid() == 0xdead_beef
  ));
  assert!(matches!(
    effects.first(),
    Some(AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Connected { .. }))
  ));
}

#[test]
fn accept_handshake_response_sets_last_used_to_acceptance_time() {
  let mut a = new_association();
  let started = a.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let response = HandshakeRsp::new(sample_remote_unique());

  let effects = a
    .accept_handshake_response(&response, 200, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");

  assert!(!effects.is_empty(), "first accepted response should publish lifecycle effects");
  assert!(matches!(a.state(), AssociationState::Active { established_at: 200, last_used_at: 200, .. }));
  assert_eq!(a.last_used_at(), Some(200));
}

#[test]
fn accept_handshake_response_refreshes_last_used_for_active_association() {
  let mut a = new_association();
  let started = a.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let response = HandshakeRsp::new(sample_remote_unique());
  let initial_effects =
    a.accept_handshake_response(&response, 200, &mut NoopInstrument).expect("initial response should be accepted");
  assert!(!initial_effects.is_empty(), "initial response should complete the handshake");

  let refresh_effects = a
    .accept_handshake_response(&response, 260, &mut NoopInstrument)
    .expect("same-origin response should refresh activity");

  assert!(refresh_effects.is_empty(), "refreshing last-used should not republish lifecycle events");
  assert!(matches!(a.state(), AssociationState::Active { established_at: 200, last_used_at: 260, .. }));
  assert_eq!(a.last_used_at(), Some(260));
}

#[test]
fn record_handshake_activity_updates_last_used_without_changing_remote_identity() {
  let mut a = new_association();
  let started = a.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let response = HandshakeRsp::new(sample_remote_unique());
  let effects = a
    .accept_handshake_response(&response, 200, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  assert!(!effects.is_empty(), "handshake response should complete the handshake");

  a.record_handshake_activity(275);

  assert!(matches!(
    a.state(),
    AssociationState::Active {
      remote_node,
      established_at: 200,
      last_used_at: 275
    } if remote_node.system() == "remote-sys" && remote_node.uid() == 0xdead_beef
  ));
  assert_eq!(a.last_used_at(), Some(275));
}

#[test]
fn liveness_probe_due_only_when_active_idle_interval_elapsed() {
  let mut active = new_association();
  let started = active.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let response = HandshakeRsp::new(sample_remote_unique());
  let effects = active
    .accept_handshake_response(&response, 200, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  assert!(!effects.is_empty(), "handshake response should complete the handshake");

  assert!(!active.is_liveness_probe_due(249, 50), "probe should not be due before the idle interval");
  assert!(active.is_liveness_probe_due(250, 50), "probe should be due when idle interval elapsed");

  let mut handshaking = new_association();
  let started = handshaking.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  assert!(!handshaking.is_liveness_probe_due(10_000, 50), "non-active association must not request liveness probes");
}

#[test]
fn accept_handshake_response_rejects_unexpected_origin_without_state_change() {
  let mut a = new_association();
  let started = a.associate(sample_endpoint(), 100, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let response = HandshakeRsp::new(other_remote_unique());

  let result = a.accept_handshake_response(&response, 200, &mut NoopInstrument);

  assert!(result.is_err(), "response from a different remote address must be rejected");
  assert!(matches!(a.state(), AssociationState::Handshaking { started_at: 100, .. }));
}

#[test]
fn handshaking_timeout_transitions_to_gated_with_lifecycle() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  let effects = a.handshake_timed_out(100, Some(500), &mut NoopInstrument);
  assert!(a.state().is_gated());
  // Publish Gated lifecycle (deferred queue empty → no DiscardEnvelopes).
  assert_eq!(effects.len(), 1);
  assert!(matches!(effects.first(), Some(AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Gated { .. }))));
}

#[test]
fn handshaking_timeout_with_deferred_envelopes_emits_discard() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  // Queue two deferred envelopes during handshake.
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 0);
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::User, "u2"), 0);
  assert_eq!(a.deferred_len(), 2);

  let effects = a.handshake_timed_out(100, None, &mut NoopInstrument);
  assert!(effects.iter().any(|e| matches!(e, AssociationEffect::DiscardEnvelopes { .. })));
  assert_eq!(a.deferred_len(), 0);
}

#[test]
fn active_to_quarantined_publishes_and_discards_pending() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  let response = HandshakeRsp::new(sample_remote_unique());
  let _ = a
    .accept_handshake_response(&response, 10, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");

  // Put an envelope into the send queue while Active.
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 0);
  assert!(!a.send_queue().is_empty());

  let effects = a.quarantine(QuarantineReason::new("fatal"), 20, &mut NoopInstrument);
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
fn quarantine_sets_removal_deadline_from_config() {
  let config = RemoteConfig::new("localhost").with_remove_quarantined_association_after(Duration::from_secs(5));
  let mut a = Association::from_config(sample_local(), sample_remote_addr(), &config);

  let effects = a.quarantine(QuarantineReason::new("fatal"), 20, &mut NoopInstrument);

  assert!(
    effects
      .iter()
      .any(|e| matches!(e, AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Quarantined { .. })))
  );
  assert!(matches!(a.state(), AssociationState::Quarantined { resume_at: Some(5_020), .. }));
}

#[test]
fn quarantine_removal_due_uses_configured_deadline_boundary() {
  let config = RemoteConfig::new("localhost").with_remove_quarantined_association_after(Duration::from_secs(5));
  let mut a = Association::from_config(sample_local(), sample_remote_addr(), &config);
  let effects = a.quarantine(QuarantineReason::new("fatal"), 20, &mut NoopInstrument);
  assert!(!effects.is_empty(), "quarantine should emit lifecycle effect");

  assert!(!a.is_quarantine_removal_due(5_019));
  assert!(a.is_quarantine_removal_due(5_020));
}

#[test]
fn sub_millisecond_remove_quarantined_association_after_does_not_round_to_now() {
  let config = RemoteConfig::new("localhost").with_remove_quarantined_association_after(Duration::from_nanos(1));
  let mut a = Association::from_config(sample_local(), sample_remote_addr(), &config);
  let effects = a.quarantine(QuarantineReason::new("fatal"), 20, &mut NoopInstrument);
  assert!(!effects.is_empty(), "quarantine should emit lifecycle effect");

  assert!(!a.is_quarantine_removal_due(20));
  assert!(a.is_quarantine_removal_due(21));
}

#[test]
fn recover_some_endpoint_from_gated_starts_handshake() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  time_out_handshake(&mut a, 10, None); // Gated
  assert!(a.state().is_gated());

  let effects = a.recover(Some(sample_endpoint()), 30, &mut NoopInstrument);
  assert!(matches!(a.state(), AssociationState::Handshaking { started_at: 30, .. }));
  assert!(matches!(effects.as_slice(), [AssociationEffect::StartHandshake { .. }]));
}

#[test]
fn recover_start_handshake_increments_generation() {
  let mut association = new_association();
  associate_idle(&mut association, 0);
  time_out_handshake(&mut association, 10, None);

  let effects = association.recover(Some(sample_endpoint()), 30, &mut NoopInstrument);

  assert_eq!(association.handshake_generation(), 2);
  assert!(matches!(
    effects.as_slice(),
    [AssociationEffect::StartHandshake {
      authority,
      generation,
      ..
    }] if authority == &sample_endpoint() && *generation == 2
  ));
}

#[test]
fn recover_records_started_phase() {
  let mut association = new_association();
  let mut recorder = RemotingFlightRecorder::new(4);
  associate_idle(&mut association, 0);
  time_out_handshake(&mut association, 10, None);

  let effects = association.recover(Some(sample_endpoint()), 30, &mut recorder);

  assert!(matches!(association.state(), AssociationState::Handshaking { started_at: 30, .. }));
  assert!(matches!(effects.as_slice(), [AssociationEffect::StartHandshake { .. }]));
  let events = recorder.snapshot().events().to_vec();
  assert!(matches!(
    events.as_slice(),
    [
      FlightRecorderEvent::Handshake {
        authority,
        phase: HandshakePhase::Started,
        now_ms: 30
      }
    ] if authority == "remote-sys@10.0.0.1:2552"
  ));
}

#[test]
fn recover_some_endpoint_from_quarantined_starts_handshake() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  let _ = a.quarantine(QuarantineReason::new("boom"), 10, &mut NoopInstrument);
  assert!(a.state().is_quarantined());

  let effects = a.recover(Some(sample_endpoint()), 50, &mut NoopInstrument);
  assert!(matches!(a.state(), AssociationState::Handshaking { .. }));
  assert_eq!(effects.len(), 1);
  assert!(matches!(effects[0], AssociationEffect::StartHandshake { .. }));
}

#[test]
fn recover_none_from_gated_returns_to_idle() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  time_out_handshake(&mut a, 10, None); // Gated

  let effects = a.recover(None, 20, &mut NoopInstrument);
  assert!(a.state().is_idle());
  assert!(effects.is_empty());
}

#[test]
fn recover_from_active_is_no_op() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  let response = HandshakeRsp::new(sample_remote_unique());
  let _ = a
    .accept_handshake_response(&response, 5, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  assert!(a.state().is_active());

  let effects = a.recover(Some(sample_endpoint()), 10, &mut NoopInstrument);
  assert!(a.state().is_active(), "Active state should be untouched by recover");
  assert!(effects.is_empty());
}

#[test]
fn recover_from_idle_is_no_op() {
  let mut a = new_association();
  assert!(a.state().is_idle());
  let e1 = a.recover(Some(sample_endpoint()), 10, &mut NoopInstrument);
  let e2 = a.recover(None, 10, &mut NoopInstrument);
  assert!(a.state().is_idle());
  assert!(e1.is_empty());
  assert!(e2.is_empty());
}

#[test]
fn recover_from_handshaking_is_no_op() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  let effects = a.recover(Some(sample_endpoint()), 10, &mut NoopInstrument);
  assert!(matches!(a.state(), AssociationState::Handshaking { .. }));
  assert!(effects.is_empty());
}

// ---------------------------------------------------------------------------
// enqueue semantics per state
// ---------------------------------------------------------------------------

#[test]
fn enqueue_in_active_pushes_into_send_queue() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  let response = HandshakeRsp::new(sample_remote_unique());
  let _ = a
    .accept_handshake_response(&response, 10, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");

  let effects = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 10);
  assert!(effects.is_empty());
  assert_eq!(a.send_queue().len(), 1);
  assert_eq!(a.deferred_len(), 0);
}

#[test]
fn enqueue_in_active_discards_when_user_queue_is_full() {
  let config = RemoteConfig::new("localhost").with_outbound_message_queue_size(1).with_outbound_control_queue_size(1);
  let mut a = Association::from_config(sample_local(), sample_remote_addr(), &config);
  let started = a.associate(sample_endpoint(), 0, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let response = HandshakeRsp::new(sample_remote_unique());
  let accepted = a
    .accept_handshake_response(&response, 10, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  assert!(!accepted.is_empty(), "handshake response should emit lifecycle effects");
  assert!(a.state().is_active());

  let first = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 20);
  let second = enqueue(&mut a, make_envelope(OutboundPriority::User, "u2"), 21);

  assert!(first.is_empty());
  assert_eq!(a.send_queue().len(), 1);
  assert_single_discard_with_priority(&second, OutboundPriority::User);
}

#[test]
fn enqueue_in_active_routes_matching_user_recipient_to_large_message_queue() {
  let config = RemoteConfig::new("localhost")
    .with_outbound_message_queue_size(1)
    .with_outbound_large_message_queue_size(1)
    .with_large_message_destinations(large_message_destinations("/user/large-*"));
  let mut a = active_association_from_config(&config);

  let large = enqueue(&mut a, make_envelope_to("/user/large-worker", OutboundPriority::User, "large"), 20);
  let normal = enqueue(&mut a, make_envelope_to("/user/small-worker", OutboundPriority::User, "normal"), 21);

  assert!(large.is_empty());
  assert!(normal.is_empty());
  assert_eq!(a.send_queue().len(), 2);
  let first = a.next_outbound(22, &mut NoopInstrument).expect("normal user should drain first");
  let second = a.next_outbound(23, &mut NoopInstrument).expect("large message should drain second");
  assert_eq!(payload_string(&first), "normal");
  assert_eq!(payload_string(&second), "large");
}

#[test]
fn enqueue_in_active_routes_non_matching_user_recipient_to_normal_user_queue() {
  let config = RemoteConfig::new("localhost")
    .with_outbound_message_queue_size(1)
    .with_outbound_large_message_queue_size(1)
    .with_large_message_destinations(large_message_destinations("/user/large-*"));
  let mut a = active_association_from_config(&config);

  let first = enqueue(&mut a, make_envelope_to("/user/small-a", OutboundPriority::User, "small-a"), 20);
  let second = enqueue(&mut a, make_envelope_to("/user/small-b", OutboundPriority::User, "small-b"), 21);

  assert!(first.is_empty());
  assert_single_discard_with_priority(&second, OutboundPriority::User);
}

#[test]
fn enqueue_in_active_keeps_matching_system_envelope_in_system_queue() {
  let config = RemoteConfig::new("localhost")
    .with_outbound_control_queue_size(1)
    .with_outbound_large_message_queue_size(1)
    .with_large_message_destinations(large_message_destinations("/user/large-*"));
  let mut a = active_association_from_config(&config);

  let system = enqueue(&mut a, make_envelope_to("/user/large-worker", OutboundPriority::System, "system"), 20);
  let large = enqueue(&mut a, make_envelope_to("/user/large-worker", OutboundPriority::User, "large"), 21);

  assert!(system.is_empty());
  assert!(large.is_empty());
  let first = a.next_outbound(22, &mut NoopInstrument).expect("system should drain first");
  let second = a.next_outbound(23, &mut NoopInstrument).expect("large user should drain second");
  assert_eq!(payload_string(&first), "system");
  assert_eq!(payload_string(&second), "large");
}

#[test]
fn enqueue_in_active_discards_when_large_message_queue_is_full() {
  let config = RemoteConfig::new("localhost")
    .with_outbound_large_message_queue_size(1)
    .with_large_message_destinations(large_message_destinations("/user/large-*"));
  let mut a = active_association_from_config(&config);

  let first = enqueue(&mut a, make_envelope_to("/user/large-a", OutboundPriority::User, "large-a"), 20);
  let second = enqueue(&mut a, make_envelope_to("/user/large-b", OutboundPriority::User, "large-b"), 21);

  assert!(first.is_empty());
  assert_single_discard_with_priority(&second, OutboundPriority::User);
}

#[test]
fn enqueue_in_active_quarantines_when_control_queue_is_full() {
  let config = RemoteConfig::new("localhost").with_outbound_message_queue_size(10).with_outbound_control_queue_size(1);
  let mut a = Association::from_config(sample_local(), sample_remote_addr(), &config);
  let mut recorder = RemotingFlightRecorder::new(4);
  let started = a.associate(sample_endpoint(), 0, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let response = HandshakeRsp::new(sample_remote_unique());
  let accepted = a
    .accept_handshake_response(&response, 10, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  assert!(!accepted.is_empty(), "handshake response should emit lifecycle effects");
  assert!(a.state().is_active());

  let first = a.enqueue(make_envelope(OutboundPriority::System, "s1"), 20, &mut recorder);
  assert!(first.is_empty());
  assert_eq!(a.send_queue().len(), 1);

  let second = a.enqueue(make_envelope(OutboundPriority::System, "s2"), 21, &mut recorder);

  assert!(a.state().is_quarantined(), "control queue overflow must quarantine the association");
  assert!(matches!(a.state(), AssociationState::Quarantined { resume_at: Some(3_600_021), .. }));
  assert!(
    second
      .iter()
      .any(|effect| matches!(effect, AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Quarantined { .. }))),
    "control queue overflow must publish a quarantine lifecycle event"
  );
  assert_discard_contains_priority(&second, OutboundPriority::System);
  let events = recorder.snapshot().events().to_vec();
  assert!(matches!(
    events.as_slice(),
    [
      FlightRecorderEvent::Quarantine {
        authority,
        reason,
        now_ms: 21
      }
    ] if authority == "remote-sys@10.0.0.1:2552"
      && reason == "Due to overflow of control queue, size [1]"
  ));
}

#[test]
fn enqueue_in_idle_discards_when_user_deferred_capacity_is_full() {
  let config = RemoteConfig::new("localhost").with_outbound_message_queue_size(1).with_outbound_control_queue_size(1);
  let mut a = Association::from_config(sample_local(), sample_remote_addr(), &config);

  let first = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 0);
  let second = enqueue(&mut a, make_envelope(OutboundPriority::User, "u2"), 0);

  assert!(first.is_empty());
  assert_eq!(a.deferred_len(), 1);
  assert_single_discard_with_priority(&second, OutboundPriority::User);
}

#[test]
fn enqueue_in_idle_uses_configured_control_deferred_capacity_independently_from_message_queue_size() {
  let config = RemoteConfig::new("localhost").with_outbound_message_queue_size(10).with_outbound_control_queue_size(1);
  let mut a = Association::from_config(sample_local(), sample_remote_addr(), &config);

  let first = enqueue(&mut a, make_envelope(OutboundPriority::System, "s1"), 0);
  let second = enqueue(&mut a, make_envelope(OutboundPriority::System, "s2"), 0);

  assert!(first.is_empty());
  assert_eq!(a.deferred_len(), 1);
  assert_single_discard_with_priority(&second, OutboundPriority::System);
}

#[test]
fn enqueue_in_handshaking_pushes_into_deferred() {
  let mut a = new_association();
  associate_idle(&mut a, 0);

  let effects = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 0);
  assert!(effects.is_empty());
  assert_eq!(a.deferred_len(), 1);
  assert!(a.send_queue().is_empty());
}

#[test]
fn enqueue_in_handshaking_uses_configured_control_deferred_capacity_independently_from_message_queue_size() {
  let config = RemoteConfig::new("localhost").with_outbound_message_queue_size(10).with_outbound_control_queue_size(1);
  let mut a = Association::from_config(sample_local(), sample_remote_addr(), &config);
  let started = a.associate(sample_endpoint(), 0, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");

  let first = enqueue(&mut a, make_envelope(OutboundPriority::System, "s1"), 0);
  let second = enqueue(&mut a, make_envelope(OutboundPriority::System, "s2"), 0);

  assert!(first.is_empty());
  assert_eq!(a.deferred_len(), 1);
  assert_single_discard_with_priority(&second, OutboundPriority::System);
}

#[test]
fn enqueue_in_gated_pushes_into_deferred() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  let response = HandshakeRsp::new(sample_remote_unique());
  let _ = a
    .accept_handshake_response(&response, 10, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  let _ = a.gate(Some(100), 20);
  assert!(a.state().is_gated());

  let effects = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 20);
  assert!(effects.is_empty());
  assert_eq!(a.deferred_len(), 1);
}

#[test]
fn enqueue_in_gated_uses_configured_control_deferred_capacity_independently_from_message_queue_size() {
  let config = RemoteConfig::new("localhost").with_outbound_message_queue_size(10).with_outbound_control_queue_size(1);
  let mut a = Association::from_config(sample_local(), sample_remote_addr(), &config);
  let started = a.associate(sample_endpoint(), 0, &mut NoopInstrument);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let response = HandshakeRsp::new(sample_remote_unique());
  let accepted = a
    .accept_handshake_response(&response, 10, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  assert!(!accepted.is_empty(), "handshake response should emit lifecycle effects");
  let gated = a.gate(Some(100), 20);
  assert!(!gated.is_empty(), "active association should enter gated state");
  assert!(a.state().is_gated());

  let first = enqueue(&mut a, make_envelope(OutboundPriority::System, "s1"), 20);
  let second = enqueue(&mut a, make_envelope(OutboundPriority::System, "s2"), 20);

  assert!(first.is_empty());
  assert_eq!(a.deferred_len(), 1);
  assert_single_discard_with_priority(&second, OutboundPriority::System);
}

#[test]
fn enqueue_in_idle_pushes_into_deferred() {
  let mut a = new_association();
  let effects = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 0);
  assert!(effects.is_empty());
  assert_eq!(a.deferred_len(), 1);
}

#[test]
fn enqueue_in_quarantined_emits_discard_effect() {
  let mut a = new_association();
  let _ = a.quarantine(QuarantineReason::new("nope"), 0, &mut NoopInstrument);
  assert!(a.state().is_quarantined());

  let effects = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 0);
  let discards: Vec<_> = effects.iter().filter(|e| matches!(e, AssociationEffect::DiscardEnvelopes { .. })).collect();
  assert_eq!(discards.len(), 1);
  // Nothing should have been deferred / enqueued.
  assert_eq!(a.deferred_len(), 0);
  assert!(a.send_queue().is_empty());
}

#[test]
fn deferred_envelopes_flush_on_handshake_accepted() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 0);
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::User, "u2"), 0);

  let response = HandshakeRsp::new(sample_remote_unique());
  let effects = a
    .accept_handshake_response(&response, 10, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
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
  associate_idle(&mut a, 0);
  let response = HandshakeRsp::new(sample_remote_unique());
  let _ = a
    .accept_handshake_response(&response, 10, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 10);
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::System, "s1"), 10);
  assert_eq!(a.total_outbound_len(), 2);

  let first = a.next_outbound(0, &mut NoopInstrument).expect("first");
  assert!(matches!(first.priority(), OutboundPriority::System));
  let second = a.next_outbound(0, &mut NoopInstrument).expect("second");
  assert!(matches!(second.priority(), OutboundPriority::User));
  assert!(a.next_outbound(0, &mut NoopInstrument).is_none());
}

#[test]
fn system_envelope_receives_redelivery_sequence_but_user_envelope_does_not() {
  let mut a = active_association_from_config(&RemoteConfig::new("127.0.0.1"));

  let _ = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 10);
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::System, "s1"), 10);

  let system = a.next_outbound(11, &mut NoopInstrument).expect("system envelope");
  assert_eq!(system.redelivery_sequence(), Some(1));
  let user = a.next_outbound(12, &mut NoopInstrument).expect("user envelope");
  assert_eq!(user.redelivery_sequence(), None);
}

#[test]
fn cumulative_ack_removes_pending_system_envelopes() {
  let mut a = active_association_from_config(&RemoteConfig::new("127.0.0.1"));

  let _ = enqueue(&mut a, make_envelope(OutboundPriority::System, "s1"), 10);
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::System, "s2"), 10);
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::System, "s3"), 10);

  let effects = a.apply_ack(&AckPdu::new(2, 2, 0), 20);
  assert!(effects.is_empty());
  let resend = a.apply_ack(&AckPdu::new(3, 2, 0b1), 21);
  assert_resend_payloads(&resend, &["s3"]);
}

#[test]
fn nack_bitmap_selects_missing_pending_envelope_for_resend() {
  let mut a = active_association_from_config(&RemoteConfig::new("127.0.0.1"));

  let _ = enqueue(&mut a, make_envelope(OutboundPriority::System, "s1"), 10);
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::System, "s2"), 10);
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::System, "s3"), 10);
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::System, "s4"), 10);

  let effects = a.apply_ack(&AckPdu::new(4, 1, 0b10), 20);
  assert_resend_payloads(&effects, &["s3"]);
}

#[test]
fn inbound_system_sequence_tracking_generates_ack_and_suppresses_duplicate() {
  let mut a = active_association_from_config(&RemoteConfig::new("127.0.0.1"));

  let (deliver_first, first_effects) = a.observe_inbound_system_envelope(1, 10);
  assert!(deliver_first);
  assert_ack(&first_effects, 1, 1, 0);

  let (deliver_duplicate, duplicate_effects) = a.observe_inbound_system_envelope(1, 11);
  assert!(!deliver_duplicate);
  assert_ack(&duplicate_effects, 1, 1, 0);
}

#[test]
fn inbound_system_gap_generates_nack_bitmap_then_advances_when_gap_arrives() {
  let mut a = active_association_from_config(&RemoteConfig::new("127.0.0.1"));

  let (deliver_first, _) = a.observe_inbound_system_envelope(1, 10);
  assert!(deliver_first);
  let (deliver_gap, gap_effects) = a.observe_inbound_system_envelope(3, 11);
  assert!(deliver_gap);
  assert_ack(&gap_effects, 3, 1, 0b1);

  let (deliver_missing, filled_effects) = a.observe_inbound_system_envelope(2, 12);
  assert!(deliver_missing);
  assert_ack(&filled_effects, 2, 3, 0);
}

#[test]
fn apply_backpressure_propagates_to_send_queue() {
  let mut a = new_association();
  associate_idle(&mut a, 0);
  let response = HandshakeRsp::new(sample_remote_unique());
  let _ = a
    .accept_handshake_response(&response, 10, &mut NoopInstrument)
    .expect("matching handshake response should be accepted");
  let _ = enqueue(&mut a, make_envelope(OutboundPriority::User, "u1"), 10);

  a.apply_backpressure(BackpressureSignal::Apply, CorrelationId::nil(), 0, &mut NoopInstrument);
  assert!(a.send_queue().is_user_paused());
  assert!(a.next_outbound(0, &mut NoopInstrument).is_none(), "user lane should be paused");

  a.apply_backpressure(BackpressureSignal::Release, CorrelationId::nil(), 0, &mut NoopInstrument);
  assert!(!a.send_queue().is_user_paused());
  let env = a.next_outbound(0, &mut NoopInstrument).expect("released");
  assert!(matches!(env.priority(), OutboundPriority::User));
}

#[test]
fn instrumented_association_methods_record_hooks_in_order() {
  let mut association = new_association();
  let mut recorder = RemotingFlightRecorder::new(8);

  let started = association.associate(sample_endpoint(), 10, &mut recorder);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let response = HandshakeRsp::new(sample_remote_unique());
  let accepted = association
    .accept_handshake_response(&response, 20, &mut recorder)
    .expect("matching handshake response should be accepted");
  assert!(!accepted.is_empty(), "first handshake response should publish lifecycle effects");
  association.apply_backpressure(BackpressureSignal::Apply, CorrelationId::nil(), 30, &mut recorder);
  let quarantined = association.quarantine(QuarantineReason::new("fatal"), 40, &mut recorder);
  assert!(!quarantined.is_empty(), "quarantine should emit lifecycle effects");

  let events = recorder.snapshot().events().to_vec();
  assert!(matches!(
    events.as_slice(),
    [
      FlightRecorderEvent::Handshake {
        authority,
        phase: HandshakePhase::Started,
        now_ms: 10
      },
      FlightRecorderEvent::Handshake {
        authority: accepted_authority,
        phase: HandshakePhase::Accepted,
        now_ms: 20
      },
      FlightRecorderEvent::Backpressure {
        authority: backpressure_authority,
        signal: BackpressureSignal::Apply,
        correlation_id,
        now_ms: 30
      },
      FlightRecorderEvent::Quarantine {
        authority: quarantined_authority,
        reason,
        now_ms: 40
      }
    ] if authority == "remote-sys@10.0.0.1:2552"
      && accepted_authority == "remote-sys@10.0.0.1:2552"
      && backpressure_authority == "remote-sys@10.0.0.1:2552"
      && quarantined_authority == "remote-sys@10.0.0.1:2552"
      && *correlation_id == CorrelationId::nil()
      && reason == "fatal"
  ));
}

#[test]
fn instrumented_timeout_records_timed_out_phase() {
  let mut association = new_association();
  let mut recorder = RemotingFlightRecorder::new(4);

  let started = association.associate(sample_endpoint(), 10, &mut recorder);
  assert!(!started.is_empty(), "associate should emit StartHandshake");
  let timed_out = association.handshake_timed_out(20, Some(50), &mut recorder);
  assert!(!timed_out.is_empty(), "timeout should emit lifecycle effects");

  let events = recorder.snapshot().events().to_vec();
  assert!(matches!(
    events.as_slice(),
    [
      FlightRecorderEvent::Handshake {
        phase: HandshakePhase::Started,
        now_ms: 10,
        ..
      },
      FlightRecorderEvent::Handshake {
        authority,
        phase: HandshakePhase::Rejected,
        now_ms: 20
      }
    ] if authority == "remote-sys@10.0.0.1:2552"
  ));
}

fn assert_single_discard_with_priority(effects: &[AssociationEffect], priority: OutboundPriority) {
  let discard = effects
    .iter()
    .find(|effect| matches!(effect, AssociationEffect::DiscardEnvelopes { .. }))
    .expect("queue overflow should discard the rejected envelope");
  if let AssociationEffect::DiscardEnvelopes { reason, envelopes } = discard {
    assert!(!reason.message().is_empty());
    assert_eq!(envelopes.len(), 1);
    assert_eq!(envelopes[0].priority(), priority);
  }
}

fn assert_discard_contains_priority(effects: &[AssociationEffect], priority: OutboundPriority) {
  let discard = effects
    .iter()
    .find(|effect| matches!(effect, AssociationEffect::DiscardEnvelopes { .. }))
    .expect("queue overflow should discard envelopes");
  if let AssociationEffect::DiscardEnvelopes { reason, envelopes } = discard {
    assert!(!reason.message().is_empty());
    assert!(
      envelopes.iter().any(|envelope| envelope.priority() == priority),
      "discarded envelopes should contain the rejected priority"
    );
  }
}

fn assert_resend_payloads(effects: &[AssociationEffect], expected: &[&str]) {
  let resend =
    effects.iter().find(|effect| matches!(effect, AssociationEffect::ResendEnvelopes { .. })).expect("resend effect");
  if let AssociationEffect::ResendEnvelopes { envelopes } = resend {
    let actual = envelopes.iter().map(payload_string).collect::<Vec<_>>();
    assert_eq!(actual, expected);
  }
}

fn assert_ack(effects: &[AssociationEffect], sequence_number: u64, cumulative_ack: u64, nack_bitmap: u64) {
  let ack = effects.iter().find(|effect| matches!(effect, AssociationEffect::SendAck { .. })).expect("ack effect");
  if let AssociationEffect::SendAck { pdu } = ack {
    assert_eq!(pdu.sequence_number(), sequence_number);
    assert_eq!(pdu.cumulative_ack(), cumulative_ack);
    assert_eq!(pdu.nack_bitmap(), nack_bitmap);
  }
}
