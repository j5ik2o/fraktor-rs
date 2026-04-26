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
  transport::TransportEndpoint,
  wire::{AckPdu, EnvelopePdu, HandshakePdu, HandshakeReq, HandshakeRsp},
};
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};
use tokio::sync::{
  mpsc,
  oneshot::{self, Sender},
};

#[path = "../test_support.rs"]
mod test_support;

use test_support::EventHarness;

use crate::std::{
  association_runtime::{
    apply_effects_in_place, association_registry::AssociationRegistry, association_shared::AssociationShared,
    handshake_driver::HandshakeDriver, inbound_dispatch::run_inbound_dispatch,
    system_message_delivery::SystemMessageDeliveryState,
  },
  tcp_transport::{InboundFrameEvent, WireFrame},
};

// ---------------------------------------------------------------------------
// AssociationShared 共有ハンドル
// ---------------------------------------------------------------------------

fn sample_association() -> Association {
  let local = UniqueAddress::new(Address::new("local-sys", "127.0.0.1", 2551), 1);
  let remote = Address::new("remote-sys", "10.0.0.1", 2552);
  Association::new(local, remote)
}

fn new_event_harness() -> EventHarness {
  let harness = EventHarness::new();
  // EventHarness は購読を維持するため ActorSystem を保持する。association_runtime
  // 側では直接 system を使わないため、test-only module ごとの dead_code 判定を避ける。
  let _system = harness.system();
  harness
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
  let s1 = state.record_send(sample_envelope_pdu(1)).unwrap();
  let s2 = state.record_send(sample_envelope_pdu(2)).unwrap();
  let s3 = state.record_send(sample_envelope_pdu(3)).unwrap();
  assert_eq!(s1, 1);
  assert_eq!(s2, 2);
  assert_eq!(s3, 3);
  assert_eq!(state.next_sequence(), 4);
  assert_eq!(state.pending_len(), 3);
}

#[test]
fn system_message_delivery_window_full_returns_none() {
  let mut state = SystemMessageDeliveryState::new(2);
  assert_eq!(state.record_send(sample_envelope_pdu(1)), Some(1));
  assert_eq!(state.record_send(sample_envelope_pdu(2)), Some(2));
  // ウィンドウ満杯時は次の record_send が None を返す。
  assert!(state.record_send(sample_envelope_pdu(3)).is_none());
  assert!(state.is_window_full());
}

#[test]
fn system_message_delivery_apply_ack_drops_acknowledged() {
  let mut state = SystemMessageDeliveryState::new(10);
  assert_eq!(state.record_send(sample_envelope_pdu(1)), Some(1));
  assert_eq!(state.record_send(sample_envelope_pdu(2)), Some(2));
  assert_eq!(state.record_send(sample_envelope_pdu(3)), Some(3));
  assert_eq!(state.pending_len(), 3);

  let ack = AckPdu::new(2, 2, 0);
  state.apply_ack(&ack);
  assert_eq!(state.cumulative_ack(), 2);
  assert_eq!(state.pending_len(), 1, "1 and 2 should be acked");
}

#[test]
fn system_message_delivery_apply_ack_is_monotonic() {
  let mut state = SystemMessageDeliveryState::new(10);
  assert_eq!(state.record_send(sample_envelope_pdu(1)), Some(1));
  assert_eq!(state.record_send(sample_envelope_pdu(2)), Some(2));
  state.apply_ack(&AckPdu::new(2, 2, 0));
  // 古い ack の小さい累積値で状態を巻き戻してはならない。
  state.apply_ack(&AckPdu::new(1, 1, 0));
  assert_eq!(state.cumulative_ack(), 2);
}

// ---------------------------------------------------------------------------
// HandshakeDriver タイムアウト制御
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn handshake_driver_fires_after_timeout_and_marks_gated() {
  let harness = new_event_harness();
  let association = {
    let mut assoc = sample_association();
    let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
    let effects = assoc.associate(endpoint, 0);
    assert!(!effects.is_empty(), "associate should emit StartHandshake");
    assoc
  };
  let shared = AssociationShared::new(association);

  let mut driver = HandshakeDriver::new();
  driver.arm(shared.clone(), Instant::now(), Duration::from_millis(10), harness.publisher().clone());
  tokio::task::yield_now().await;
  tokio::time::advance(Duration::from_millis(10)).await;
  tokio::task::yield_now().await;

  shared.with_write(|assoc| {
    assert!(assoc.state().is_gated(), "handshake driver should have transitioned the association into Gated");
  });
  let events = harness.events();
  assert!(has_remoting_lifecycle_event(&events, |event| matches!(
    event,
    RemotingLifecycleEvent::Gated {
      authority,
      correlation_id
    } if authority == "remote-sys@10.0.0.1:2552" && *correlation_id == CorrelationId::nil()
  )));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn handshake_driver_cancel_prevents_firing() {
  let harness = new_event_harness();
  let association = {
    let mut assoc = sample_association();
    let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
    let effects = assoc.associate(endpoint, 0);
    assert!(!effects.is_empty(), "associate should emit StartHandshake");
    assoc
  };
  let shared = AssociationShared::new(association);

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
  let events = harness.events();
  assert!(!has_remoting_lifecycle_event(&events, |event| matches!(event, RemotingLifecycleEvent::Gated { .. })));
}

// ---------------------------------------------------------------------------
// outbound_loop / inbound_dispatch（配線のスモークテスト）
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn outbound_loop_drains_active_association() {
  use fraktor_remote_core_rs::core::transport::{RemoteTransport, TransportError};

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

  let mut association = sample_association();
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let associate_effects = association.associate(endpoint, 0);
  assert!(!associate_effects.is_empty(), "associate should emit StartHandshake");
  let connected_effects = association.handshake_accepted(RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1), 1);
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

  tokio::time::timeout(Duration::from_secs(1), sent_rx)
    .await
    .expect("outbound loop should send before the test timeout")
    .expect("send completion should be delivered");

  task.abort();
  let join_error = task.await.expect_err("aborted outbound loop should return JoinError");
  assert!(join_error.is_cancelled(), "outbound loop task should be cancelled by abort");

  let sent_len = sent.with_lock(|sent| sent.len());
  assert_eq!(sent_len, 1, "outbound loop should have drained one envelope");
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
  let harness = new_event_harness();
  // associate 済み、かつ handshake 未完了のため deferred envelope を保持する association を作る。
  let mut association = sample_association();
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let associate_effects = association.associate(endpoint, 0);
  assert!(!associate_effects.is_empty(), "associate should emit StartHandshake");
  assert!(association.enqueue(deferred_envelope()).is_empty(), "handshaking enqueue should defer without effects");
  assert!(association.enqueue(deferred_envelope()).is_empty(), "handshaking enqueue should defer without effects");
  // handshake_accepted 前は send queue から drain できないことを確認する。
  assert!(association.next_outbound().is_none(), "deferred envelopes must not be drainable before handshake_accepted");

  // handshake を完了し、返された effects をその場で適用する。
  // production 側（`inbound_dispatch::run_inbound_dispatch`）が守る契約である。
  let effects = association.handshake_accepted(RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1), 1);
  apply_effects_in_place(&mut association, effects, harness.publisher());

  // deferred envelope が失われず、active の send queue へ再投入されたことを確認する。
  assert!(association.next_outbound().is_some(), "first deferred envelope must be re-enqueued");
  assert!(association.next_outbound().is_some(), "second deferred envelope must be re-enqueued");
  assert!(association.next_outbound().is_none(), "no further envelopes expected");
  let events = harness.events();
  assert!(has_remoting_lifecycle_event(&events, |event| matches!(
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
}

#[test]
fn handshake_timed_out_effects_drop_deferred_envelopes_observably() {
  let harness = new_event_harness();
  // associate 済み、かつ handshake 未完了のため deferred envelope を保持する association を作る。
  let mut association = sample_association();
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let associate_effects = association.associate(endpoint, 0);
  assert!(!associate_effects.is_empty(), "associate should emit StartHandshake");
  assert!(association.enqueue(deferred_envelope()).is_empty(), "handshaking enqueue should defer without effects");

  // timeout 遷移を発火し、effects をその場で適用する。
  // `handshake_driver::HandshakeDriver` が守る契約である。
  let effects = association.handshake_timed_out(0, None);
  apply_effects_in_place(&mut association, effects, harness.publisher());

  // timeout path で deferred envelope を破棄するため、状態は Gated で send queue は空になる。
  assert!(association.state().is_gated(), "handshake_timed_out should have moved the association to Gated");
  assert!(association.next_outbound().is_none(), "Gated state must not surface envelopes from next_outbound");
  let events = harness.events();
  assert!(has_remoting_lifecycle_event(&events, |event| matches!(event, RemotingLifecycleEvent::Gated { .. })));
}

#[test]
fn handshake_accepted_with_no_deferred_envelopes_is_a_noop() {
  let harness = new_event_harness();
  // flush 対象が空でも、effects 適用で panic せず phantom envelope も生成しない。
  let mut association = sample_association();
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let associate_effects = association.associate(endpoint, 0);
  assert!(!associate_effects.is_empty(), "associate should emit StartHandshake");

  let effects = association.handshake_accepted(RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1), 1);
  apply_effects_in_place(&mut association, effects, harness.publisher());

  assert!(association.next_outbound().is_none());
}

#[test]
fn apply_effects_in_place_publishes_lifecycle_events_to_event_stream() {
  let harness = new_event_harness();
  let mut association = sample_association();
  let effects = vec![AssociationEffect::PublishLifecycle(RemotingLifecycleEvent::Quarantined {
    authority:      String::from("remote-sys@10.0.0.1:2552"),
    reason:         String::from("test quarantine"),
    correlation_id: CorrelationId::from_u128(99),
  })];

  apply_effects_in_place(&mut association, effects, harness.publisher());

  let events = harness.events();
  assert!(has_remoting_lifecycle_event(&events, |event| matches!(
    event,
    RemotingLifecycleEvent::Quarantined {
      authority,
      reason,
      correlation_id
    } if authority == "remote-sys@10.0.0.1:2552"
      && reason == "test quarantine"
      && *correlation_id == CorrelationId::from_u128(99)
  )));
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_publishes_connected_lifecycle_with_req_origin() {
  let harness = new_event_harness();
  let association = {
    let mut assoc = sample_association();
    let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
    let effects = assoc.associate(endpoint, 0);
    assert!(!effects.is_empty(), "associate should emit StartHandshake");
    assoc
  };
  let shared = AssociationShared::new(association);
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(InboundFrameEvent {
    peer:  String::from("10.0.0.1:2552"),
    frame: WireFrame::Handshake(HandshakePdu::Req(HandshakeReq::new(
      String::from("remote-sys"),
      String::from("10.0.0.1"),
      2552,
      1,
    ))),
  })
  .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch(rx, shared, || 200, harness.publisher().clone()).await;

  let events = harness.events();
  assert!(has_remoting_lifecycle_event(&events, |event| matches!(
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
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_discards_handshake_for_different_association() {
  let harness = new_event_harness();
  let association = {
    let mut assoc = sample_association();
    let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
    let effects = assoc.associate(endpoint, 0);
    assert!(!effects.is_empty(), "associate should emit StartHandshake");
    assoc
  };
  let shared = AssociationShared::new(association);
  let shared_for_assert = shared.clone();
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(InboundFrameEvent {
    peer:  String::from("10.0.0.9:2552"),
    frame: WireFrame::Handshake(HandshakePdu::Req(HandshakeReq::new(
      String::from("other-sys"),
      String::from("10.0.0.9"),
      2552,
      9,
    ))),
  })
  .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch(rx, shared, || 200, harness.publisher().clone()).await;

  shared_for_assert.with_write(|assoc| {
    assert!(matches!(assoc.state(), AssociationState::Handshaking { .. }));
  });
  let events = harness.events();
  assert!(!has_remoting_lifecycle_event(&events, |event| matches!(event, RemotingLifecycleEvent::Connected { .. })));
}

#[tokio::test(flavor = "current_thread")]
async fn inbound_dispatch_publishes_connected_lifecycle_with_rsp_origin() {
  let harness = new_event_harness();
  let association = {
    let mut assoc = sample_association();
    let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
    let effects = assoc.associate(endpoint, 0);
    assert!(!effects.is_empty(), "associate should emit StartHandshake");
    assoc
  };
  let shared = AssociationShared::new(association);
  let (tx, rx) = mpsc::unbounded_channel();

  tx.send(InboundFrameEvent {
    peer:  String::from("10.0.0.1:2552"),
    frame: WireFrame::Handshake(HandshakePdu::Rsp(HandshakeRsp::new(
      String::from("remote-sys"),
      String::from("10.0.0.1"),
      2552,
      1,
    ))),
  })
  .expect("handshake frame should be sent to inbound dispatch");
  drop(tx);

  run_inbound_dispatch(rx, shared, || 200, harness.publisher().clone()).await;

  let events = harness.events();
  assert!(has_remoting_lifecycle_event(&events, |event| matches!(
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
}
