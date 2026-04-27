use core::time::Duration;
use std::time::Instant;

use bytes::Bytes;
use fraktor_actor_core_rs::core::kernel::{
  actor::{actor_path::ActorPathParser, messaging::AnyMessage},
  event::stream::{CorrelationId, EventStreamEvent, RemotingLifecycleEvent},
};
use fraktor_remote_core_rs::core::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::{Association, AssociationEffect, AssociationState, QuarantineReason},
  envelope::{OutboundEnvelope, OutboundPriority},
  transport::{RemoteTransport, TransportEndpoint, TransportError},
  watcher::WatcherCommand,
  wire::{AckPdu, ControlPdu, EnvelopePdu, HandshakePdu, HandshakeReq, HandshakeRsp},
};
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};
use tokio::sync::{
  mpsc::{self, UnboundedReceiver},
  oneshot::{self, Sender},
};

use crate::std::{
  association_runtime::{
    InboundQuarantineCheck, RestartCounter, apply_effects_in_place, association_registry::AssociationRegistry,
    association_shared::AssociationShared, handshake_driver::HandshakeDriver, inbound_dispatch::run_inbound_dispatch,
    system_message_delivery::SystemMessageDeliveryState,
  },
  tcp_transport::{InboundFrameEvent, WireFrame},
  tests::test_support::EventHarness,
};

// ---------------------------------------------------------------------------
// AssociationShared 共有ハンドル
// ---------------------------------------------------------------------------

fn sample_association() -> Association {
  sample_association_for(Address::new("remote-sys", "10.0.0.1", 2552))
}

fn sample_association_for(remote: Address) -> Association {
  let local = UniqueAddress::new(Address::new("local-sys", "127.0.0.1", 2551), 1);
  Association::new(local, remote)
}

fn handshaking_association() -> Association {
  handshaking_association_for(Address::new("remote-sys", "10.0.0.1", 2552))
}

fn handshaking_association_for(remote: Address) -> Association {
  let mut assoc = sample_association_for(remote.clone());
  let endpoint = TransportEndpoint::new(remote.to_string());
  let effects = assoc.associate(endpoint, 0);
  assert!(!effects.is_empty(), "associate should emit StartHandshake");
  assoc
}

fn active_association_for(remote: Address) -> Association {
  let mut assoc = handshaking_association_for(remote.clone());
  let response = HandshakeRsp::new(remote_unique(remote.system(), remote.host(), remote.port(), 1));
  let effects = assoc.accept_handshake_response(&response, 1).expect("matching handshake response should be accepted");
  assert!(!effects.is_empty(), "handshake response should activate the association");
  assoc
}

fn quarantined_association_for(remote: Address) -> Association {
  let mut assoc = active_association_for(remote);
  let effects = assoc.quarantine(QuarantineReason::new("test quarantine"), 2);
  assert!(!effects.is_empty(), "quarantine should emit lifecycle effects");
  assert!(assoc.state().is_quarantined(), "association must be quarantined for this test");
  assoc
}

fn remote_address(system: &str, host: &str, port: u16) -> Address {
  Address::new(system, host, port)
}

fn local_address() -> Address {
  Address::new("local-sys", "127.0.0.1", 2551)
}

fn local_unique() -> UniqueAddress {
  UniqueAddress::new(local_address(), 1)
}

fn remote_unique(system: &str, host: &str, port: u16, uid: u64) -> UniqueAddress {
  UniqueAddress::new(remote_address(system, host, port), uid)
}

fn remote_handshake_req(system: &str, host: &str, port: u16, uid: u64) -> WireFrame {
  remote_handshake_req_to(system, host, port, uid, local_address())
}

fn remote_handshake_req_to(system: &str, host: &str, port: u16, uid: u64, to: Address) -> WireFrame {
  WireFrame::Handshake(HandshakePdu::Req(HandshakeReq::new(remote_unique(system, host, port, uid), to)))
}

fn remote_handshake_rsp(system: &str, host: &str, port: u16, uid: u64) -> WireFrame {
  WireFrame::Handshake(HandshakePdu::Rsp(HandshakeRsp::new(remote_unique(system, host, port, uid))))
}

type SentHandshakes = SharedLock<Vec<(Address, HandshakePdu)>>;
type SentControls = SharedLock<Vec<(Address, ControlPdu)>>;
type SubmittedWatcherCommands = SharedLock<Vec<WatcherCommand>>;

fn new_sent_handshakes() -> SentHandshakes {
  SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new())
}

fn new_sent_controls() -> SentControls {
  SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new())
}

fn new_submitted_watcher_commands() -> SubmittedWatcherCommands {
  SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new())
}

fn sent_handshakes(sent: &SentHandshakes) -> Vec<(Address, HandshakePdu)> {
  sent.with_lock(|items| items.clone())
}

fn sent_control_frames(sent: &SentControls) -> Vec<(Address, ControlPdu)> {
  sent.with_lock(|items| items.clone())
}

fn registry_with(remote: Address, association: Association) -> AssociationRegistry {
  let mut registry = AssociationRegistry::new();
  registry.insert(UniqueAddress::new(remote, 0), AssociationShared::new(association));
  registry
}

fn inbound_event_from(remote: &Address, frame: WireFrame) -> InboundFrameEvent {
  InboundFrameEvent { peer: format!("{}:{}", remote.host(), remote.port()), frame }
}

fn inbound_event_from_peer(peer: &str, frame: WireFrame) -> InboundFrameEvent {
  InboundFrameEvent { peer: peer.into(), frame }
}

fn handshake_send_probe(
  sent: SentHandshakes,
) -> impl FnMut(&Address, HandshakePdu) -> Result<(), TransportError> + Send + 'static {
  move |remote, pdu| {
    sent.with_lock(|items| items.push((remote.clone(), pdu)));
    Ok(())
  }
}

fn control_send_probe(
  sent: SentControls,
) -> impl FnMut(&Address, ControlPdu) -> Result<(), TransportError> + Send + 'static {
  move |remote, pdu| {
    sent.with_lock(|items| items.push((remote.clone(), pdu)));
    Ok(())
  }
}

fn watcher_submit_probe(
  submitted: SubmittedWatcherCommands,
) -> impl FnMut(WatcherCommand) -> Result<(), Box<WatcherCommand>> + Send + 'static {
  move |command| {
    submitted.with_lock(|items| items.push(command));
    Ok(())
  }
}

async fn run_inbound_dispatch_with_response_probe(
  rx: UnboundedReceiver<InboundFrameEvent>,
  registry: AssociationRegistry,
  now_ms: u64,
  harness: &EventHarness,
  sent: SentHandshakes,
) {
  let sent_controls = new_sent_controls();
  let submitted_commands = new_submitted_watcher_commands();
  run_inbound_dispatch(
    rx,
    registry,
    move || now_ms,
    harness.publisher().clone(),
    (
      local_unique(),
      handshake_send_probe(sent),
      control_send_probe(sent_controls),
      watcher_submit_probe(submitted_commands),
    ),
  )
  .await;
}

async fn run_inbound_dispatch_with_control_probes(
  rx: UnboundedReceiver<InboundFrameEvent>,
  registry: AssociationRegistry,
  now_ms: u64,
  harness: &EventHarness,
  sent_controls: SentControls,
  submitted_commands: SubmittedWatcherCommands,
) {
  run_inbound_dispatch(
    rx,
    registry,
    move || now_ms,
    harness.publisher().clone(),
    (
      local_unique(),
      handshake_send_probe(new_sent_handshakes()),
      control_send_probe(sent_controls),
      watcher_submit_probe(submitted_commands),
    ),
  )
  .await;
}

async fn run_inbound_dispatch_with_all_probes(
  rx: UnboundedReceiver<InboundFrameEvent>,
  registry: AssociationRegistry,
  now_ms: u64,
  harness: &EventHarness,
  sent_handshakes: SentHandshakes,
  sent_controls: SentControls,
  submitted_commands: SubmittedWatcherCommands,
) {
  run_inbound_dispatch(
    rx,
    registry,
    move || now_ms,
    harness.publisher().clone(),
    (
      local_unique(),
      handshake_send_probe(sent_handshakes),
      control_send_probe(sent_controls),
      watcher_submit_probe(submitted_commands),
    ),
  )
  .await;
}

fn has_remoting_lifecycle_event(
  events: &[EventStreamEvent],
  expected: impl Fn(&RemotingLifecycleEvent) -> bool,
) -> bool {
  events.iter().any(|event| match event {
    | EventStreamEvent::RemotingLifecycle(lifecycle) => expected(lifecycle),
    | _ => false,
  })
}

#[test]
fn association_shared_with_write_drives_state_machine() {
  let shared = AssociationShared::new(sample_association());
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let effects = shared.with_write(|assoc| assoc.associate(endpoint, 100));
  assert!(!effects.is_empty(), "associate should emit StartHandshake");
  // 初回遷移後は Handshaking 状態になる。
  shared.with_write(|assoc| assert!(matches!(assoc.state(), AssociationState::Handshaking { .. })));
}

#[test]
fn association_shared_clone_shares_state() {
  let a = AssociationShared::new(sample_association());
  let b = a.clone();
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  a.with_write(|assoc| {
    let effects = assoc.associate(endpoint, 0);
    assert!(!effects.is_empty(), "associate should emit StartHandshake");
  });
  b.with_write(|assoc| {
    assert!(matches!(assoc.state(), AssociationState::Handshaking { .. }));
  });
}

// ---------------------------------------------------------------------------
// AssociationRegistry 登録表
// ---------------------------------------------------------------------------

#[test]
fn registry_insert_and_lookup_works() {
  let mut reg = AssociationRegistry::new();
  let addr = UniqueAddress::new(Address::new("sys", "host", 1), 1);
  let shared = AssociationShared::new(sample_association());
  reg.insert(addr.clone(), shared);
  assert_eq!(reg.len(), 1);
  assert!(reg.get(&addr).is_some());
}

#[test]
fn registry_remove_drops_the_entry() {
  let mut reg = AssociationRegistry::new();
  let addr = UniqueAddress::new(Address::new("sys", "host", 1), 1);
  reg.insert(addr.clone(), AssociationShared::new(sample_association()));
  let removed = reg.remove(&addr);
  assert!(removed.is_some());
  assert!(reg.is_empty());
}

#[test]
fn registry_iter_yields_all_entries() {
  let mut reg = AssociationRegistry::new();
  let a = UniqueAddress::new(Address::new("sys", "host-a", 1), 1);
  let b = UniqueAddress::new(Address::new("sys", "host-b", 2), 2);
  reg.insert(a.clone(), AssociationShared::new(sample_association()));
  reg.insert(b.clone(), AssociationShared::new(sample_association()));
  let collected: Vec<_> = reg.iter().map(|(addr, _)| addr.clone()).collect();
  assert_eq!(collected.len(), 2);
  assert!(collected.contains(&a));
  assert!(collected.contains(&b));
}

#[test]
fn registry_get_by_remote_address_matches_without_requiring_uid() {
  let mut reg = AssociationRegistry::new();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  reg.insert(UniqueAddress::new(remote.clone(), 42), AssociationShared::new(sample_association_for(remote.clone())));

  let found = reg.get_by_remote_address(&remote).expect("registry should resolve by remote address");

  found.with_write(|assoc| {
    assert_eq!(assoc.remote(), &remote);
  });
}

#[test]
fn registry_get_by_remote_address_returns_none_for_unknown_peer() {
  let mut reg = AssociationRegistry::new();
  let known = remote_address("remote-sys", "10.0.0.1", 2552);
  let unknown = remote_address("unknown-sys", "10.0.0.9", 2552);
  reg.insert(UniqueAddress::new(known.clone(), 42), AssociationShared::new(sample_association_for(known)));

  assert!(reg.get_by_remote_address(&unknown).is_none());
}

// ---------------------------------------------------------------------------
// InboundQuarantineCheck 検疫フィルタ
// ---------------------------------------------------------------------------

#[test]
fn inbound_quarantine_check_allows_envelope_from_active_peer() {
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), active_association_for(remote.clone()));
  let event = inbound_event_from(&remote, WireFrame::Envelope(sample_envelope_pdu(1)));

  let allowed = InboundQuarantineCheck::allows(&registry, &event);

  assert!(allowed, "active peer envelope must continue into inbound dispatch");
}

#[test]
fn inbound_quarantine_check_drops_envelope_from_quarantined_peer() {
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), quarantined_association_for(remote.clone()));
  let event = inbound_event_from(&remote, WireFrame::Envelope(sample_envelope_pdu(1)));

  let allowed = InboundQuarantineCheck::allows(&registry, &event);

  assert!(!allowed, "quarantined peer envelope must be dropped before normal inbound dispatch");
}

#[test]
fn inbound_quarantine_check_drops_heartbeat_from_quarantined_peer() {
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), quarantined_association_for(remote.clone()));
  let event = inbound_event_from(&remote, WireFrame::Control(ControlPdu::Heartbeat { authority: remote.to_string() }));

  let allowed = InboundQuarantineCheck::allows(&registry, &event);

  assert!(!allowed, "quarantined peer heartbeat must be dropped before normal inbound dispatch");
}

#[test]
fn inbound_quarantine_check_drops_heartbeat_response_from_quarantined_peer() {
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), quarantined_association_for(remote.clone()));
  let event = inbound_event_from(
    &remote,
    WireFrame::Control(ControlPdu::HeartbeatResponse { authority: remote.to_string(), uid: 42 }),
  );

  let allowed = InboundQuarantineCheck::allows(&registry, &event);

  assert!(!allowed, "quarantined peer heartbeat response must be dropped before normal inbound dispatch");
}

#[test]
fn inbound_quarantine_check_drops_handshake_from_quarantined_peer() {
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), quarantined_association_for(remote.clone()));
  let event = inbound_event_from(&remote, remote_handshake_req("remote-sys", "10.0.0.1", 2552, 1));

  let allowed = InboundQuarantineCheck::allows(&registry, &event);

  assert!(!allowed, "quarantined peer handshake must be dropped before normal inbound dispatch");
}

#[test]
fn inbound_quarantine_check_drops_quarantine_notice_from_quarantined_peer() {
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), quarantined_association_for(remote.clone()));
  let event = inbound_event_from(
    &remote,
    WireFrame::Control(ControlPdu::Quarantine {
      authority: remote.to_string(),
      reason:    Some("test quarantine".into()),
    }),
  );

  let allowed = InboundQuarantineCheck::allows(&registry, &event);

  assert!(!allowed, "quarantined peer quarantine notices must be dropped before normal inbound dispatch");
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_drops_quarantined_peer_before_side_effects() {
  let harness = EventHarness::new();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), quarantined_association_for(remote.clone()));
  let sent_handshake_frames = new_sent_handshakes();
  let sent_controls = new_sent_controls();
  let submitted_commands = new_submitted_watcher_commands();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(inbound_event_from(
    &remote,
    WireFrame::Control(ControlPdu::HeartbeatResponse { authority: remote.to_string(), uid: 42 }),
  ))
  .expect("heartbeat response frame should be sent to inbound dispatch");
  tx.send(inbound_event_from(&remote, remote_handshake_req("remote-sys", "10.0.0.1", 2552, 1)))
    .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_all_probes(
    rx,
    registry,
    333,
    &harness,
    sent_handshake_frames.clone(),
    sent_controls.clone(),
    submitted_commands.clone(),
  )
  .await;

  assert!(
    sent_handshakes(&sent_handshake_frames).is_empty(),
    "quarantined peer handshake must not emit handshake responses"
  );
  assert!(
    sent_control_frames(&sent_controls).is_empty(),
    "quarantined peer control frame must not emit control responses"
  );
  assert!(
    submitted_commands.with_lock(|items| items.is_empty()),
    "quarantined peer heartbeat response must not submit watcher commands"
  );
  harness.events_with(|events| {
    assert!(
      !has_remoting_lifecycle_event(events, |event| matches!(event, RemotingLifecycleEvent::Connected { .. })),
      "quarantined peer frames must not publish Connected lifecycle events"
    );
  });
}

// ---------------------------------------------------------------------------
// SystemMessageDeliveryState 配送状態
// ---------------------------------------------------------------------------

fn sample_envelope_pdu(seq_for_payload: u64) -> EnvelopePdu {
  EnvelopePdu::new("/user/x".into(), None, seq_for_payload, 0, 0, Bytes::from_static(b"data"))
}

#[test]
fn system_message_delivery_assigns_monotonic_sequence_numbers() {
  let mut state = SystemMessageDeliveryState::new(100);
  let s1 = state.record_send(sample_envelope_pdu(1), 100).unwrap();
  let s2 = state.record_send(sample_envelope_pdu(2), 110).unwrap();
  let s3 = state.record_send(sample_envelope_pdu(3), 120).unwrap();
  assert_eq!(s1, 1);
  assert_eq!(s2, 2);
  assert_eq!(s3, 3);
  assert_eq!(state.next_sequence(), 4);
  assert_eq!(state.pending_len(), 3);
}

#[test]
fn system_message_delivery_window_full_returns_none() {
  let mut state = SystemMessageDeliveryState::new(2);
  assert_eq!(state.record_send(sample_envelope_pdu(1), 100), Some(1));
  assert_eq!(state.record_send(sample_envelope_pdu(2), 110), Some(2));
  // ウィンドウ満杯時は次の record_send が None を返す。
  assert!(state.record_send(sample_envelope_pdu(3), 120).is_none());
  assert!(state.is_window_full());
}

#[test]
fn system_message_delivery_apply_ack_drops_acknowledged() {
  let mut state = SystemMessageDeliveryState::new(10);
  assert_eq!(state.record_send(sample_envelope_pdu(1), 100), Some(1));
  assert_eq!(state.record_send(sample_envelope_pdu(2), 110), Some(2));
  assert_eq!(state.record_send(sample_envelope_pdu(3), 120), Some(3));
  assert_eq!(state.pending_len(), 3);

  let ack = AckPdu::new(2, 2, 0);
  state.apply_ack(&ack);
  assert_eq!(state.cumulative_ack(), 2);
  assert_eq!(state.pending_len(), 1, "1 and 2 should be acked");
}

#[test]
fn system_message_delivery_apply_ack_is_monotonic() {
  let mut state = SystemMessageDeliveryState::new(10);
  assert_eq!(state.record_send(sample_envelope_pdu(1), 100), Some(1));
  assert_eq!(state.record_send(sample_envelope_pdu(2), 110), Some(2));
  state.apply_ack(&AckPdu::new(2, 2, 0));
  // 古い ack の小さい累積値で状態を巻き戻してはならない。
  state.apply_ack(&AckPdu::new(1, 1, 0));
  assert_eq!(state.cumulative_ack(), 2);
}

#[test]
fn system_message_delivery_due_retransmissions_respect_resend_interval() {
  let mut state = SystemMessageDeliveryState::new(10);
  let first = sample_envelope_pdu(1);
  let second = sample_envelope_pdu(2);

  assert_eq!(state.record_send(first.clone(), 1_000), Some(1));
  assert_eq!(state.record_send(second.clone(), 1_020), Some(2));

  assert!(state.due_retransmissions(1_049, 50).is_empty());
  assert_eq!(state.due_retransmissions(1_050, 50), vec![(1, first.clone())]);
  assert_eq!(state.due_retransmissions(1_070, 50), vec![(1, first), (2, second)]);
}

#[test]
fn system_message_delivery_mark_retransmitted_updates_send_time() {
  let mut state = SystemMessageDeliveryState::new(10);
  let envelope = sample_envelope_pdu(1);

  assert_eq!(state.record_send(envelope.clone(), 1_000), Some(1));
  assert_eq!(state.due_retransmissions(1_050, 50), vec![(1, envelope.clone())]);

  assert!(state.mark_retransmitted(1, 1_050));
  assert!(state.due_retransmissions(1_099, 50).is_empty());
  assert_eq!(state.due_retransmissions(1_100, 50), vec![(1, envelope)]);
}

#[test]
fn system_message_delivery_mark_retransmitted_returns_false_for_unknown_sequence() {
  let mut state = SystemMessageDeliveryState::new(10);

  assert_eq!(state.record_send(sample_envelope_pdu(1), 1_000), Some(1));

  assert!(!state.mark_retransmitted(99, 1_050));
}

#[test]
fn system_message_delivery_nacked_pending_returns_pending_bitmap_matches_only() {
  let mut state = SystemMessageDeliveryState::new(10);
  let first = sample_envelope_pdu(1);
  let second = sample_envelope_pdu(2);
  let third = sample_envelope_pdu(3);
  let fourth = sample_envelope_pdu(4);

  assert_eq!(state.record_send(first, 1_000), Some(1));
  assert_eq!(state.record_send(second.clone(), 1_010), Some(2));
  assert_eq!(state.record_send(third, 1_020), Some(3));
  assert_eq!(state.record_send(fourth.clone(), 1_030), Some(4));

  let ack = AckPdu::new(5, 1, 0b1101);
  state.apply_ack(&ack);

  assert_eq!(state.nacked_pending(&ack), vec![(2, second), (4, fourth)]);
}

// ---------------------------------------------------------------------------
// HandshakeDriver タイムアウト制御
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn handshake_driver_fires_after_timeout_and_marks_gated() {
  let harness = EventHarness::new();
  let shared = AssociationShared::new(handshaking_association());

  let mut driver = HandshakeDriver::new();
  driver.arm(shared.clone(), Instant::now(), Duration::from_millis(10), harness.publisher().clone());
  tokio::task::yield_now().await;
  tokio::time::advance(Duration::from_millis(10)).await;
  tokio::task::yield_now().await;

  shared.with_write(|assoc| {
    assert!(assoc.state().is_gated(), "handshake driver should have transitioned the association into Gated");
  });
  harness.events_with(|events| {
    assert!(has_remoting_lifecycle_event(events, |event| matches!(
      event,
      RemotingLifecycleEvent::Gated {
        authority,
        correlation_id
      } if authority == "remote-sys@10.0.0.1:2552" && *correlation_id == CorrelationId::nil()
    )));
  });
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn handshake_driver_cancel_prevents_firing() {
  let harness = EventHarness::new();
  let shared = AssociationShared::new(handshaking_association());

  let mut driver = HandshakeDriver::new();
  driver.arm(shared.clone(), Instant::now(), Duration::from_millis(50), harness.publisher().clone());
  tokio::task::yield_now().await;
  driver.cancel();
  tokio::time::advance(Duration::from_millis(50)).await;
  tokio::task::yield_now().await;

  shared.with_write(|assoc| {
    assert!(
      matches!(assoc.state(), AssociationState::Handshaking { .. }),
      "cancelled driver must not transition state"
    );
  });
  harness.events_with(|events| {
    assert!(!has_remoting_lifecycle_event(events, |event| matches!(event, RemotingLifecycleEvent::Gated { .. })));
  });
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn handshake_driver_arm_aborts_previous_timeout() {
  // 二重 arm された場合、最初の timeout が abort されず生き残ると、
  // re-arm 後の新しい deadline より前に古い timeout が fire して
  // association を Gated に落としてしまう。`arm` は前回の timeout を
  // 必ず abort してから新しい task を spawn しなければならない。
  let harness = EventHarness::new();
  let shared = AssociationShared::new(handshaking_association());

  let mut driver = HandshakeDriver::new();
  driver.arm(shared.clone(), Instant::now(), Duration::from_millis(10), harness.publisher().clone());
  tokio::task::yield_now().await;
  // Re-arm with a longer deadline before the first one fires.
  driver.arm(shared.clone(), Instant::now(), Duration::from_millis(100), harness.publisher().clone());
  tokio::task::yield_now().await;

  // Advance past the *first* timeout but well before the second.
  tokio::time::advance(Duration::from_millis(20)).await;
  tokio::task::yield_now().await;

  shared.with_write(|assoc| {
    assert!(
      matches!(assoc.state(), AssociationState::Handshaking { .. }),
      "the first timeout must have been aborted by re-arm and not gate the association"
    );
  });
  harness.events_with(|events| {
    assert!(
      !has_remoting_lifecycle_event(events, |event| matches!(event, RemotingLifecycleEvent::Gated { .. })),
      "no Gated event should be observed before the second timeout fires"
    );
  });

  driver.cancel();
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn handshake_driver_retries_handshake_req_while_handshaking() {
  let shared = AssociationShared::new(handshaking_association());
  let sent = new_sent_handshakes();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);

  let mut driver = HandshakeDriver::new();
  driver.arm_retry(
    shared,
    local_unique(),
    remote.clone(),
    Duration::from_millis(10),
    handshake_send_probe(sent.clone()),
  );
  tokio::task::yield_now().await;
  tokio::time::advance(Duration::from_millis(10)).await;
  tokio::task::yield_now().await;

  let recorded = sent_handshakes(&sent);
  assert_eq!(recorded.len(), 1, "retry tick should send exactly one handshake request");
  assert_eq!(recorded[0].0, remote);
  assert_eq!(recorded[0].1, HandshakePdu::Req(HandshakeReq::new(local_unique(), remote)));

  driver.cancel();
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn handshake_driver_cancel_prevents_retry_tick() {
  let shared = AssociationShared::new(handshaking_association());
  let sent = new_sent_handshakes();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);

  let mut driver = HandshakeDriver::new();
  driver.arm_retry(shared, local_unique(), remote, Duration::from_millis(10), handshake_send_probe(sent.clone()));
  tokio::task::yield_now().await;
  driver.cancel();
  tokio::time::advance(Duration::from_millis(10)).await;
  tokio::task::yield_now().await;

  assert!(sent_handshakes(&sent).is_empty(), "cancelled retry task must not send handshake requests");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn handshake_driver_injects_handshake_req_while_active() {
  let mut association = handshaking_association();
  let response = HandshakeRsp::new(remote_unique("remote-sys", "10.0.0.1", 2552, 1));
  let effects = association.accept_handshake_response(&response, 0).expect("handshake should complete");
  assert!(!effects.is_empty(), "initial handshake should emit lifecycle effects");
  let shared = AssociationShared::new(association);
  let sent = new_sent_handshakes();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);

  let mut driver = HandshakeDriver::new();
  driver.arm_inject(
    shared,
    local_unique(),
    remote.clone(),
    Duration::from_millis(25),
    handshake_send_probe(sent.clone()),
  );
  tokio::task::yield_now().await;
  tokio::time::advance(Duration::from_millis(25)).await;
  tokio::task::yield_now().await;

  let recorded = sent_handshakes(&sent);
  assert_eq!(recorded.len(), 1, "inject tick should send exactly one handshake request");
  assert_eq!(recorded[0].0, remote);
  assert_eq!(recorded[0].1, HandshakePdu::Req(HandshakeReq::new(local_unique(), remote)));

  driver.cancel();
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn handshake_driver_sends_liveness_probe_when_active_association_is_idle() {
  let mut association = handshaking_association();
  let response = HandshakeRsp::new(remote_unique("remote-sys", "10.0.0.1", 2552, 1));
  let effects = association.accept_handshake_response(&response, 0).expect("handshake should complete");
  assert!(!effects.is_empty(), "initial handshake should emit lifecycle effects");
  let shared = AssociationShared::new(association);
  let sent = new_sent_handshakes();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let now_ms = SharedLock::new_with_driver::<DefaultMutex<_>>(0_u64);
  let now_ms_for_driver = now_ms.clone();

  let mut driver = HandshakeDriver::new();
  driver.arm_liveness_probe(
    shared,
    local_unique(),
    remote.clone(),
    Duration::from_millis(10),
    move || now_ms_for_driver.with_lock(|value| *value),
    handshake_send_probe(sent.clone()),
  );
  now_ms.with_lock(|value| *value = 10);
  tokio::task::yield_now().await;
  tokio::time::advance(Duration::from_millis(10)).await;
  tokio::task::yield_now().await;

  let recorded = sent_handshakes(&sent);
  assert_eq!(recorded.len(), 1, "idle active association should receive one liveness probe");
  assert_eq!(recorded[0].0, remote);
  assert_eq!(recorded[0].1, HandshakePdu::Req(HandshakeReq::new(local_unique(), remote)));

  driver.cancel();
}

// ---------------------------------------------------------------------------
// outbound_loop / inbound_dispatch（配線のスモークテスト）
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn outbound_loop_drains_active_association() {
  use crate::std::association_runtime::outbound_loop::run_outbound_loop;

  // 送信要求された envelope をすべて記録する transport。
  struct CapturingTransport {
    sent:        SharedLock<Vec<OutboundEnvelope>>,
    sent_signal: Option<Sender<()>>,
    addresses:   Vec<Address>,
  }

  impl RemoteTransport for CapturingTransport {
    fn start(&mut self) -> Result<(), TransportError> {
      Ok(())
    }

    fn shutdown(&mut self) -> Result<(), TransportError> {
      Ok(())
    }

    fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), TransportError> {
      self.sent.with_lock(|sent| sent.push(envelope));
      if let Some(sent_signal) = self.sent_signal.take() {
        sent_signal.send(()).expect("send completion receiver should be alive");
      }
      Ok(())
    }

    fn addresses(&self) -> &[Address] {
      &self.addresses
    }

    fn default_address(&self) -> Option<&Address> {
      self.addresses.first()
    }

    fn local_address_for_remote(&self, _remote: &Address) -> Option<&Address> {
      self.addresses.first()
    }

    fn quarantine(
      &mut self,
      _address: &Address,
      _uid: Option<u64>,
      _reason: QuarantineReason,
    ) -> Result<(), TransportError> {
      Ok(())
    }
  }

  let mut association = handshaking_association();
  let response = HandshakeRsp::new(remote_unique("remote-sys", "10.0.0.1", 2552, 1));
  let connected_effects =
    association.accept_handshake_response(&response, 1).expect("matching handshake response should be accepted");
  assert!(!connected_effects.is_empty(), "handshake_accepted should emit Connected lifecycle");
  // system 優先度の envelope を投入する。
  let path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/x").unwrap();
  let envelope = OutboundEnvelope::new(
    path,
    None,
    AnyMessage::new(()),
    OutboundPriority::System,
    RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
    CorrelationId::nil(),
  );
  let enqueue_effects = association.enqueue(envelope);
  assert!(enqueue_effects.is_empty(), "active enqueue should only append to the send queue");

  let shared = AssociationShared::new(association);
  let sent = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::<OutboundEnvelope>::new());
  let (sent_tx, sent_rx) = oneshot::channel();
  let transport = SharedLock::new_with_driver::<DefaultMutex<_>>(CapturingTransport {
    sent:        sent.clone(),
    sent_signal: Some(sent_tx),
    addresses:   vec![Address::new("local-sys", "127.0.0.1", 2551)],
  });

  let task_shared = shared.clone();
  let task_transport = transport.clone();
  let task = tokio::spawn(async move {
    run_outbound_loop(task_shared, task_transport).await;
  });

  tokio::task::yield_now().await;
  tokio::time::advance(Duration::from_millis(1)).await;
  tokio::task::yield_now().await;

  tokio::time::timeout(Duration::from_secs(5), sent_rx)
    .await
    .expect("outbound loop should send before the test timeout")
    .expect("send completion should be delivered");

  task.abort();
  let join_error = task.await.expect_err("aborted outbound loop should return JoinError");
  assert!(join_error.is_cancelled(), "outbound loop task should be cancelled by abort");

  let sent_len = sent.with_lock(|sent| sent.len());
  assert_eq!(sent_len, 1, "outbound loop should have drained one envelope");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn outbound_loop_waits_backoff_before_reconnect_and_recovers_association() {
  use crate::std::association_runtime::{ReconnectBackoffPolicy, run_outbound_loop_with_reconnect};

  let mut association = handshaking_association();
  let response = HandshakeRsp::new(remote_unique("remote-sys", "10.0.0.1", 2552, 1));
  let connected_effects =
    association.accept_handshake_response(&response, 1).expect("matching handshake response should be accepted");
  assert!(!connected_effects.is_empty(), "handshake response should activate the association");
  assert!(association.enqueue(deferred_envelope()).is_empty(), "active enqueue should append to the send queue");

  let shared = AssociationShared::new(association);
  let sends = SharedLock::new_with_driver::<DefaultMutex<_>>(0_u32);
  let transport = SharedLock::new_with_driver::<DefaultMutex<_>>(FailingTransport::new(
    TransportError::ConnectionClosed,
    sends.clone(),
  ));
  let reconnects = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::<Address>::new());
  let reconnects_for_closure = reconnects.clone();
  let policy =
    ReconnectBackoffPolicy::new(Duration::from_millis(20), Duration::from_millis(100), Duration::from_millis(100), 3);
  let reconnect = move |remote: Address| {
    let reconnects = reconnects_for_closure.clone();
    async move {
      reconnects.with_lock(|items| items.push(remote.clone()));
      Ok(TransportEndpoint::new(remote.to_string()))
    }
  };

  let task_shared = shared.clone();
  let task_transport = transport.clone();
  let harness = EventHarness::new();
  let event_publisher = harness.publisher().clone();
  let task = tokio::spawn(async move {
    run_outbound_loop_with_reconnect(task_shared, task_transport, policy, event_publisher, reconnect).await
  });

  tokio::task::yield_now().await;
  tokio::time::advance(Duration::from_millis(1)).await;
  tokio::task::yield_now().await;

  assert_eq!(sends.with_lock(|count| *count), 1, "first send should be attempted before reconnect");
  shared.with_write(|assoc| {
    assert!(assoc.state().is_gated(), "send failure should gate the association before reconnect");
  });
  assert!(
    reconnects.with_lock(|items| items.is_empty()),
    "reconnect must not run before the configured backoff elapses"
  );

  tokio::time::advance(Duration::from_millis(20)).await;
  tokio::task::yield_now().await;

  assert_eq!(reconnects.with_lock(|items| items.clone()), vec![remote_address("remote-sys", "10.0.0.1", 2552)]);
  shared.with_write(|assoc| {
    assert!(
      matches!(assoc.state(), AssociationState::Handshaking { .. }),
      "successful reconnect should recover the association into handshaking"
    );
  });

  task.abort();
  let join_error = task.await.expect_err("aborted reconnect loop should return JoinError");
  assert!(join_error.is_cancelled(), "reconnect loop task should be cancelled by abort");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn outbound_loop_returns_send_failure_when_restart_budget_is_exhausted() {
  use crate::std::association_runtime::{ReconnectBackoffPolicy, run_outbound_loop_with_reconnect};

  let mut association = handshaking_association();
  let response = HandshakeRsp::new(remote_unique("remote-sys", "10.0.0.1", 2552, 1));
  let connected_effects =
    association.accept_handshake_response(&response, 1).expect("matching handshake response should be accepted");
  assert!(!connected_effects.is_empty(), "handshake response should activate the association");
  assert!(association.enqueue(deferred_envelope()).is_empty(), "active enqueue should append to the send queue");

  let shared = AssociationShared::new(association);
  let sends = SharedLock::new_with_driver::<DefaultMutex<_>>(0_u32);
  let transport =
    SharedLock::new_with_driver::<DefaultMutex<_>>(FailingTransport::new(TransportError::SendFailed, sends.clone()));
  let reconnects = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::<Address>::new());
  let reconnects_for_closure = reconnects.clone();
  let policy =
    ReconnectBackoffPolicy::new(Duration::from_millis(20), Duration::from_millis(100), Duration::from_millis(100), 0);
  let reconnect = move |remote: Address| {
    let reconnects = reconnects_for_closure.clone();
    async move {
      reconnects.with_lock(|items| items.push(remote.clone()));
      Ok(TransportEndpoint::new(remote.to_string()))
    }
  };

  let harness = EventHarness::new();
  let result =
    run_outbound_loop_with_reconnect(shared.clone(), transport, policy, harness.publisher().clone(), reconnect).await;

  assert_eq!(result, Err(TransportError::SendFailed));
  assert_eq!(sends.with_lock(|count| *count), 1, "one send attempt should consume the zero restart budget");
  assert!(reconnects.with_lock(|items| items.is_empty()), "restart budget exhaustion must not call reconnect");
  shared.with_write(|assoc| {
    assert!(assoc.state().is_gated(), "budget exhaustion should leave the association observably gated");
  });
}

#[test]
fn restart_counter_allows_restarts_until_budget_is_consumed_in_window() {
  let mut counter = RestartCounter::new(2, Duration::from_millis(100));

  let first = counter.restart(1_000);
  let second = counter.restart(1_050);

  assert!(first, "first restart in a fresh window should be allowed");
  assert!(second, "max_restarts represents the number of allowed restarts in the active window");
  assert_eq!(counter.count(), 2, "count reflects the active restart-timeout window");
}

#[test]
fn restart_counter_rejects_restart_after_budget_is_consumed_inside_window() {
  let mut counter = RestartCounter::new(2, Duration::from_millis(100));

  assert!(counter.restart(1_000), "first restart should consume one budget slot");
  assert!(counter.restart(1_050), "second restart should consume the last budget slot");
  let third = counter.restart(1_099);

  assert!(!third, "third restart inside the same restart-timeout window must be rejected");
}

#[test]
fn restart_counter_resets_budget_after_restart_timeout_window_expires() {
  let mut counter = RestartCounter::new(2, Duration::from_millis(100));

  assert!(counter.restart(1_000), "first restart should open the restart-timeout window");
  assert!(counter.restart(1_050), "second restart should consume the active window budget");
  assert!(!counter.restart(1_099), "budget is still exhausted before the window expires");
  let after_deadline = counter.restart(1_101);

  assert!(after_deadline, "restart budget must reset only after the restart-timeout window expires");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn outbound_loop_treats_not_started_as_shutdown_without_reconnect() {
  use crate::std::association_runtime::{ReconnectBackoffPolicy, run_outbound_loop_with_reconnect};

  let mut association = handshaking_association();
  let response = HandshakeRsp::new(remote_unique("remote-sys", "10.0.0.1", 2552, 1));
  let connected_effects =
    association.accept_handshake_response(&response, 1).expect("matching handshake response should be accepted");
  assert!(!connected_effects.is_empty(), "handshake response should activate the association");
  assert!(association.enqueue(deferred_envelope()).is_empty(), "active enqueue should append to the send queue");

  let shared = AssociationShared::new(association);
  let sends = SharedLock::new_with_driver::<DefaultMutex<_>>(0_u32);
  let transport =
    SharedLock::new_with_driver::<DefaultMutex<_>>(FailingTransport::new(TransportError::NotStarted, sends.clone()));
  let reconnects = SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::<Address>::new());
  let reconnects_for_closure = reconnects.clone();
  let policy =
    ReconnectBackoffPolicy::new(Duration::from_millis(20), Duration::from_millis(100), Duration::from_millis(100), 3);
  let reconnect = move |remote: Address| {
    let reconnects = reconnects_for_closure.clone();
    async move {
      reconnects.with_lock(|items| items.push(remote.clone()));
      Ok(TransportEndpoint::new(remote.to_string()))
    }
  };

  let harness = EventHarness::new();
  let result =
    run_outbound_loop_with_reconnect(shared.clone(), transport, policy, harness.publisher().clone(), reconnect).await;

  assert_eq!(result, Ok(()));
  assert_eq!(sends.with_lock(|count| *count), 1, "shutdown path should observe the pending send once");
  assert!(
    reconnects.with_lock(|items| items.is_empty()),
    "NotStarted is shutdown, not a reconnectable connection failure"
  );
  shared.with_write(|assoc| {
    assert!(assoc.state().is_active(), "shutdown must not gate an otherwise active association");
  });
}

struct FailingTransport {
  failure:   TransportError,
  sends:     SharedLock<u32>,
  addresses: Vec<Address>,
}

impl FailingTransport {
  fn new(failure: TransportError, sends: SharedLock<u32>) -> Self {
    Self { failure, sends, addresses: vec![Address::new("local-sys", "127.0.0.1", 2551)] }
  }
}

impl RemoteTransport for FailingTransport {
  fn start(&mut self) -> Result<(), TransportError> {
    Ok(())
  }

  fn shutdown(&mut self) -> Result<(), TransportError> {
    Ok(())
  }

  fn send(&mut self, _envelope: OutboundEnvelope) -> Result<(), TransportError> {
    self.sends.with_lock(|count| *count += 1);
    Err(self.failure.clone())
  }

  fn addresses(&self) -> &[Address] {
    &self.addresses
  }

  fn default_address(&self) -> Option<&Address> {
    self.addresses.first()
  }

  fn local_address_for_remote(&self, _remote: &Address) -> Option<&Address> {
    self.addresses.first()
  }

  fn quarantine(
    &mut self,
    _address: &Address,
    _uid: Option<u64>,
    _reason: QuarantineReason,
  ) -> Result<(), TransportError> {
    Ok(())
  }
}

// ---------------------------------------------------------------------------
// effect_application — handshake 完了時に deferred envelope が失われないことを確認する。
// effect vector の取りこぼしに対する回帰テスト。
// ---------------------------------------------------------------------------

fn deferred_envelope() -> OutboundEnvelope {
  let path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/buffered").unwrap();
  OutboundEnvelope::new(
    path,
    None,
    AnyMessage::new(()),
    OutboundPriority::User,
    RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
    CorrelationId::nil(),
  )
}

#[test]
fn handshake_accepted_effects_re_enqueue_deferred_envelopes() {
  let harness = EventHarness::new();
  // associate 済み、かつ handshake 未完了のため deferred envelope を保持する association を作る。
  let mut association = handshaking_association();
  assert!(association.enqueue(deferred_envelope()).is_empty(), "handshaking enqueue should defer without effects");
  assert!(association.enqueue(deferred_envelope()).is_empty(), "handshaking enqueue should defer without effects");
  // handshake_accepted 前は send queue から drain できないことを確認する。
  assert!(association.next_outbound().is_none(), "deferred envelopes must not be drainable before handshake_accepted");

  // handshake を完了し、返された effects をその場で適用する。
  // production 側（`inbound_dispatch::run_inbound_dispatch`）が守る契約である。
  let response = HandshakeRsp::new(remote_unique("remote-sys", "10.0.0.1", 2552, 1));
  let effects =
    association.accept_handshake_response(&response, 1).expect("matching handshake response should be accepted");
  apply_effects_in_place(&mut association, effects, harness.publisher());

  // deferred envelope が失われず、active の send queue へ再投入されたことを確認する。
  assert!(association.next_outbound().is_some(), "first deferred envelope must be re-enqueued");
  assert!(association.next_outbound().is_some(), "second deferred envelope must be re-enqueued");
  assert!(association.next_outbound().is_none(), "no further envelopes expected");
  harness.events_with(|events| {
    assert!(has_remoting_lifecycle_event(events, |event| matches!(
      event,
      RemotingLifecycleEvent::Connected {
        authority,
        remote_system,
        remote_uid,
        correlation_id
      } if authority == "remote-sys@10.0.0.1:2552"
        && remote_system == "remote-sys"
        && *remote_uid == 1
        && *correlation_id == CorrelationId::nil()
    )));
  });
}

#[test]
fn handshake_timed_out_effects_drop_deferred_envelopes_observably() {
  let harness = EventHarness::new();
  // associate 済み、かつ handshake 未完了のため deferred envelope を保持する association を作る。
  let mut association = handshaking_association();
  assert!(association.enqueue(deferred_envelope()).is_empty(), "handshaking enqueue should defer without effects");

  // timeout 遷移を発火し、effects をその場で適用する。
  // `handshake_driver::HandshakeDriver` が守る契約である。
  let effects = association.handshake_timed_out(0, None);
  apply_effects_in_place(&mut association, effects, harness.publisher());

  // timeout path で deferred envelope を破棄するため、状態は Gated で send queue は空になる。
  assert!(association.state().is_gated(), "handshake_timed_out should have moved the association to Gated");
  assert!(association.next_outbound().is_none(), "Gated state must not surface envelopes from next_outbound");
  harness.events_with(|events| {
    assert!(has_remoting_lifecycle_event(events, |event| matches!(event, RemotingLifecycleEvent::Gated { .. })));
  });
}

#[test]
fn handshake_accepted_with_no_deferred_envelopes_is_a_noop() {
  let harness = EventHarness::new();
  // flush 対象が空でも、effects 適用で panic せず phantom envelope も生成しない。
  let mut association = handshaking_association();

  let response = HandshakeRsp::new(remote_unique("remote-sys", "10.0.0.1", 2552, 1));
  let effects =
    association.accept_handshake_response(&response, 1).expect("matching handshake response should be accepted");
  apply_effects_in_place(&mut association, effects, harness.publisher());

  assert!(association.next_outbound().is_none());
}

#[test]
fn apply_effects_in_place_publishes_lifecycle_events_to_event_stream() {
  let harness = EventHarness::new();
  let mut association = sample_association();
  let effects = vec![AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Quarantined {
    authority:      String::from("remote-sys@10.0.0.1:2552"),
    reason:         String::from("test quarantine"),
    correlation_id: CorrelationId::from_u128(99),
  })];

  apply_effects_in_place(&mut association, effects, harness.publisher());

  harness.events_with(|events| {
    assert!(has_remoting_lifecycle_event(events, |event| matches!(
      event,
      RemotingLifecycleEvent::Quarantined {
        authority,
        reason,
        correlation_id
      } if authority == "remote-sys@10.0.0.1:2552"
        && reason == "test quarantine"
        && *correlation_id == CorrelationId::from_u128(99)
    )));
  });
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_publishes_connected_lifecycle_with_req_origin() {
  let harness = EventHarness::new();
  let shared = AssociationShared::new(handshaking_association());
  let mut registry = AssociationRegistry::new();
  registry.insert(UniqueAddress::new(remote_address("remote-sys", "10.0.0.1", 2552), 0), shared.clone());
  let sent = new_sent_handshakes();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(InboundFrameEvent {
    peer:  String::from("10.0.0.1:2552"),
    frame: remote_handshake_req("remote-sys", "10.0.0.1", 2552, 1),
  })
  .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_response_probe(rx, registry, 200, &harness, sent).await;

  harness.events_with(|events| {
    assert!(has_remoting_lifecycle_event(events, |event| matches!(
      event,
      RemotingLifecycleEvent::Connected {
        authority,
        remote_system,
        remote_uid,
        correlation_id
      } if authority == "remote-sys@10.0.0.1:2552"
        && remote_system == "remote-sys"
        && *remote_uid == 1
        && *correlation_id == CorrelationId::nil()
    )));
  });
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_replies_to_valid_handshake_req_with_local_unique_address() {
  let harness = EventHarness::new();
  let shared = AssociationShared::new(handshaking_association());
  let mut registry = AssociationRegistry::new();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  registry.insert(UniqueAddress::new(remote.clone(), 0), shared);
  let sent = new_sent_handshakes();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(InboundFrameEvent {
    peer:  String::from("10.0.0.1:2552"),
    frame: remote_handshake_req("remote-sys", "10.0.0.1", 2552, 1),
  })
  .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_response_probe(rx, registry, 200, &harness, sent.clone()).await;

  assert_eq!(sent_handshakes(&sent), vec![(remote, HandshakePdu::Rsp(HandshakeRsp::new(local_unique())))]);
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_does_not_reply_when_association_is_gated() {
  // Association が Gated の状態で HandshakeReq を受信しても、accept_handshake_request は
  // RejectedInState を返すため HandshakeRsp は送信されない。これにより、Pekko の遮断中
  // に reconnect 試行を勝手に成功扱いしてしまう非対称な状態を防ぐ。
  let harness = EventHarness::new();
  let mut association = handshaking_association();
  let _ = association.handshake_timed_out(0, Some(500));
  assert!(association.state().is_gated(), "association must be in Gated state for this test");
  let shared = AssociationShared::new(association);
  let shared_for_assert = shared.clone();
  let mut registry = AssociationRegistry::new();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  registry.insert(UniqueAddress::new(remote.clone(), 0), shared);
  let sent = new_sent_handshakes();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(InboundFrameEvent {
    peer:  String::from("10.0.0.1:2552"),
    frame: remote_handshake_req("remote-sys", "10.0.0.1", 2552, 1),
  })
  .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_response_probe(rx, registry, 200, &harness, sent.clone()).await;

  assert!(sent_handshakes(&sent).is_empty(), "gated association must not receive a handshake response");
  shared_for_assert.with_write(|assoc| {
    assert!(assoc.state().is_gated(), "association state must remain Gated after rejected handshake");
  });
  harness.events_with(|events| {
    assert!(
      !has_remoting_lifecycle_event(events, |event| matches!(event, RemotingLifecycleEvent::Connected { .. })),
      "no Connected lifecycle should be emitted while gated"
    );
  });
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_discards_handshake_for_different_association() {
  let harness = EventHarness::new();
  let shared = AssociationShared::new(handshaking_association());
  let shared_for_assert = shared.clone();
  let mut registry = AssociationRegistry::new();
  registry.insert(UniqueAddress::new(remote_address("remote-sys", "10.0.0.1", 2552), 0), shared);
  let sent = new_sent_handshakes();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(InboundFrameEvent {
    peer:  String::from("10.0.0.9:2552"),
    frame: remote_handshake_req("other-sys", "10.0.0.9", 2552, 9),
  })
  .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_response_probe(rx, registry, 200, &harness, sent.clone()).await;

  shared_for_assert.with_write(|assoc| {
    assert!(matches!(assoc.state(), AssociationState::Handshaking { .. }));
  });
  assert!(sent_handshakes(&sent).is_empty(), "unknown peer must not receive a handshake response");
  harness.events_with(|events| {
    assert!(!has_remoting_lifecycle_event(events, |event| matches!(event, RemotingLifecycleEvent::Connected { .. })));
  });
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_rejects_handshake_req_for_unexpected_local_address() {
  let harness = EventHarness::new();
  let shared = AssociationShared::new(handshaking_association());
  let shared_for_assert = shared.clone();
  let mut registry = AssociationRegistry::new();
  registry.insert(UniqueAddress::new(remote_address("remote-sys", "10.0.0.1", 2552), 0), shared);
  let sent = new_sent_handshakes();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(InboundFrameEvent {
    peer:  String::from("10.0.0.1:2552"),
    frame: remote_handshake_req_to("remote-sys", "10.0.0.1", 2552, 1, Address::new("other-local", "127.0.0.2", 2551)),
  })
  .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_response_probe(rx, registry, 200, &harness, sent.clone()).await;

  shared_for_assert.with_write(|assoc| {
    assert!(matches!(assoc.state(), AssociationState::Handshaking { .. }));
  });
  assert!(sent_handshakes(&sent).is_empty(), "invalid request must not receive a handshake response");
  harness.events_with(|events| {
    assert!(!has_remoting_lifecycle_event(events, |event| matches!(event, RemotingLifecycleEvent::Connected { .. })));
  });
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_publishes_connected_lifecycle_with_rsp_origin() {
  let harness = EventHarness::new();
  let shared = AssociationShared::new(handshaking_association());
  let mut registry = AssociationRegistry::new();
  registry.insert(UniqueAddress::new(remote_address("remote-sys", "10.0.0.1", 2552), 0), shared.clone());
  let sent = new_sent_handshakes();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(InboundFrameEvent {
    peer:  String::from("10.0.0.1:2552"),
    frame: remote_handshake_rsp("remote-sys", "10.0.0.1", 2552, 1),
  })
  .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_response_probe(rx, registry, 200, &harness, sent.clone()).await;

  assert!(sent_handshakes(&sent).is_empty(), "handshake response must not trigger another response");

  harness.events_with(|events| {
    assert!(has_remoting_lifecycle_event(events, |event| matches!(
      event,
      RemotingLifecycleEvent::Connected {
        authority,
        remote_system,
        remote_uid,
        correlation_id
      } if authority == "remote-sys@10.0.0.1:2552"
        && remote_system == "remote-sys"
        && *remote_uid == 1
        && *correlation_id == CorrelationId::nil()
    )));
  });
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_routes_handshake_to_matching_registered_association_only() {
  let harness = EventHarness::new();
  let remote_a = remote_address("remote-a", "10.0.0.1", 2552);
  let remote_b = remote_address("remote-b", "10.0.0.2", 2553);
  let shared_a = AssociationShared::new(handshaking_association_for(remote_a.clone()));
  let shared_b = AssociationShared::new(handshaking_association_for(remote_b.clone()));
  let mut registry = AssociationRegistry::new();
  registry.insert(UniqueAddress::new(remote_a, 0), shared_a.clone());
  registry.insert(UniqueAddress::new(remote_b, 0), shared_b.clone());
  let sent = new_sent_handshakes();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(InboundFrameEvent {
    peer:  String::from("10.0.0.2:2553"),
    frame: remote_handshake_req("remote-b", "10.0.0.2", 2553, 22),
  })
  .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_response_probe(rx, registry, 200, &harness, sent).await;

  shared_a.with_write(|assoc| {
    assert!(matches!(assoc.state(), AssociationState::Handshaking { .. }));
  });
  shared_b.with_write(|assoc| {
    assert!(matches!(
      assoc.state(),
      AssociationState::Active {
        remote_node,
        ..
      } if remote_node.system() == "remote-b" && remote_node.uid() == 22
    ));
  });
  harness.events_with(|events| {
    assert!(has_remoting_lifecycle_event(events, |event| matches!(
      event,
      RemotingLifecycleEvent::Connected {
        authority,
        remote_system,
        remote_uid,
        ..
      } if authority == "remote-b@10.0.0.2:2553" && remote_system == "remote-b" && *remote_uid == 22
    )));
    assert!(!has_remoting_lifecycle_event(events, |event| matches!(
      event,
      RemotingLifecycleEvent::Connected {
        authority,
        ..
      } if authority == "remote-a@10.0.0.1:2552"
    )));
  });
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_replies_to_heartbeat_with_local_uid() {
  let harness = EventHarness::new();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), active_association_for(remote.clone()));
  let sent_controls = new_sent_controls();
  let submitted_commands = new_submitted_watcher_commands();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(inbound_event_from(&remote, WireFrame::Control(ControlPdu::Heartbeat { authority: remote.to_string() })))
    .expect("heartbeat frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_control_probes(
    rx,
    registry,
    300,
    &harness,
    sent_controls.clone(),
    submitted_commands.clone(),
  )
  .await;

  assert_eq!(sent_control_frames(&sent_controls), vec![(remote, ControlPdu::HeartbeatResponse {
    authority: local_address().to_string(),
    uid:       local_unique().uid(),
  },)]);
  assert!(
    submitted_commands.with_lock(|items| items.is_empty()),
    "heartbeat request should not be submitted as watcher response"
  );
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_drops_heartbeat_request_when_peer_does_not_match_authority() {
  let harness = EventHarness::new();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), active_association_for(remote.clone()));
  let sent_controls = new_sent_controls();
  let submitted_commands = new_submitted_watcher_commands();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(inbound_event_from_peer(
    "10.0.0.9:2552",
    WireFrame::Control(ControlPdu::Heartbeat { authority: remote.to_string() }),
  ))
  .expect("heartbeat frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_control_probes(
    rx,
    registry,
    300,
    &harness,
    sent_controls.clone(),
    submitted_commands.clone(),
  )
  .await;

  assert!(
    sent_control_frames(&sent_controls).is_empty(),
    "mismatched heartbeat request must not receive a heartbeat response"
  );
  assert!(
    submitted_commands.with_lock(|items| items.is_empty()),
    "mismatched heartbeat request must not submit watcher commands"
  );
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_submits_heartbeat_response_to_watcher() {
  let harness = EventHarness::new();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), active_association_for(remote.clone()));
  let sent_controls = new_sent_controls();
  let submitted_commands = new_submitted_watcher_commands();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(inbound_event_from(
    &remote,
    WireFrame::Control(ControlPdu::HeartbeatResponse { authority: remote.to_string(), uid: 42 }),
  ))
  .expect("heartbeat response frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_control_probes(
    rx,
    registry,
    333,
    &harness,
    sent_controls.clone(),
    submitted_commands.clone(),
  )
  .await;

  assert!(
    sent_control_frames(&sent_controls).is_empty(),
    "heartbeat response should not trigger another control response"
  );
  let commands = submitted_commands.with_lock(|items| items.clone());
  assert_eq!(commands.len(), 1);
  assert!(matches!(
    &commands[0],
    WatcherCommand::HeartbeatResponseReceived {
      from,
      uid,
      now
    } if from == &remote && *uid == 42 && *now == 333
  ));
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_drops_heartbeat_response_when_peer_does_not_match_authority() {
  let harness = EventHarness::new();
  let remote = remote_address("remote-sys", "10.0.0.1", 2552);
  let registry = registry_with(remote.clone(), active_association_for(remote.clone()));
  let sent_controls = new_sent_controls();
  let submitted_commands = new_submitted_watcher_commands();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(inbound_event_from_peer(
    "10.0.0.9:2552",
    WireFrame::Control(ControlPdu::HeartbeatResponse { authority: remote.to_string(), uid: 42 }),
  ))
  .expect("heartbeat response frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch_with_control_probes(
    rx,
    registry,
    333,
    &harness,
    sent_controls.clone(),
    submitted_commands.clone(),
  )
  .await;

  assert!(
    sent_control_frames(&sent_controls).is_empty(),
    "mismatched heartbeat response must not trigger control responses"
  );
  assert!(
    submitted_commands.with_lock(|items| items.is_empty()),
    "mismatched heartbeat response must not be submitted to the watcher"
  );
}
