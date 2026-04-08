use core::time::Duration;
use std::time::Instant;

use bytes::Bytes;
use fraktor_actor_core_rs::core::kernel::{
  actor::{actor_path::ActorPathParser, messaging::AnyMessage},
  event::stream::CorrelationId,
};
use fraktor_remote_core_rs::{
  address::{Address, RemoteNodeId, UniqueAddress},
  association::{Association, QuarantineReason},
  envelope::{OutboundEnvelope, OutboundPriority},
  transport::TransportEndpoint,
  wire::{AckPdu, EnvelopePdu},
};

use crate::association_runtime::{
  apply_effects_in_place, association_registry::AssociationRegistry, association_shared::AssociationShared,
  handshake_driver::HandshakeDriver, system_message_delivery::SystemMessageDeliveryState,
};

// ---------------------------------------------------------------------------
// AssociationShared
// ---------------------------------------------------------------------------

fn sample_association() -> Association {
  let local = UniqueAddress::new(Address::new("local-sys", "127.0.0.1", 2551), 1);
  let remote = Address::new("remote-sys", "10.0.0.1", 2552);
  Association::new(local, remote)
}

#[test]
fn association_shared_with_write_drives_state_machine() {
  let shared = AssociationShared::new(sample_association());
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let effects = shared.with_write(|assoc| assoc.associate(endpoint, 100));
  assert!(!effects.is_empty(), "associate should emit StartHandshake");
  // The state should be Handshaking after the first transition.
  shared.with_write(|assoc| {
    assert!(matches!(assoc.state(), fraktor_remote_core_rs::association::AssociationState::Handshaking { .. }))
  });
}

#[test]
fn association_shared_clone_shares_state() {
  let a = AssociationShared::new(sample_association());
  let b = a.clone();
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  a.with_write(|assoc| {
    let _ = assoc.associate(endpoint, 0);
  });
  b.with_write(|assoc| {
    assert!(matches!(assoc.state(), fraktor_remote_core_rs::association::AssociationState::Handshaking { .. }));
  });
}

// ---------------------------------------------------------------------------
// AssociationRegistry
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
// SystemMessageDeliveryState
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
  // Window full → next record_send returns None.
  assert!(state.record_send(sample_envelope_pdu(3)).is_none());
  assert!(state.is_window_full());
}

#[test]
fn system_message_delivery_apply_ack_drops_acknowledged() {
  let mut state = SystemMessageDeliveryState::new(10);
  let _ = state.record_send(sample_envelope_pdu(1));
  let _ = state.record_send(sample_envelope_pdu(2));
  let _ = state.record_send(sample_envelope_pdu(3));
  assert_eq!(state.pending_len(), 3);

  let ack = AckPdu::new(2, 2, 0);
  state.apply_ack(&ack);
  assert_eq!(state.cumulative_ack(), 2);
  assert_eq!(state.pending_len(), 1, "1 and 2 should be acked");
}

#[test]
fn system_message_delivery_apply_ack_is_monotonic() {
  let mut state = SystemMessageDeliveryState::new(10);
  let _ = state.record_send(sample_envelope_pdu(1));
  let _ = state.record_send(sample_envelope_pdu(2));
  state.apply_ack(&AckPdu::new(2, 2, 0));
  // A stale ack with a smaller cumulative value must not regress the state.
  state.apply_ack(&AckPdu::new(1, 1, 0));
  assert_eq!(state.cumulative_ack(), 2);
}

// ---------------------------------------------------------------------------
// HandshakeDriver
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn handshake_driver_fires_after_timeout_and_marks_gated() {
  let association = {
    let mut assoc = sample_association();
    let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
    let _ = assoc.associate(endpoint, 0);
    assoc
  };
  let shared = AssociationShared::new(association);

  let mut driver = HandshakeDriver::new();
  driver.arm(shared.clone(), Instant::now(), Duration::from_millis(10));
  // Wait long enough for the timeout to fire.
  tokio::time::sleep(Duration::from_millis(60)).await;

  shared.with_write(|assoc| {
    assert!(assoc.state().is_gated(), "handshake driver should have transitioned the association into Gated");
  });
}

#[tokio::test(flavor = "current_thread")]
async fn handshake_driver_cancel_prevents_firing() {
  let association = {
    let mut assoc = sample_association();
    let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
    let _ = assoc.associate(endpoint, 0);
    assoc
  };
  let shared = AssociationShared::new(association);

  let mut driver = HandshakeDriver::new();
  driver.arm(shared.clone(), Instant::now(), Duration::from_millis(50));
  driver.cancel();
  tokio::time::sleep(Duration::from_millis(120)).await;

  shared.with_write(|assoc| {
    assert!(
      matches!(assoc.state(), fraktor_remote_core_rs::association::AssociationState::Handshaking { .. }),
      "cancelled driver must not transition state"
    );
  });
}

// ---------------------------------------------------------------------------
// outbound_loop / inbound_dispatch (smoke test compile-time wiring)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn outbound_loop_drains_active_association() {
  use core::time::Duration;
  use std::sync::{Arc, Mutex};

  use fraktor_remote_core_rs::transport::{RemoteTransport, TransportError};

  use crate::association_runtime::outbound_loop::run_outbound_loop;

  // A capturing transport that records every envelope it is asked to send.
  struct CapturingTransport {
    sent:      Arc<Mutex<Vec<OutboundEnvelope>>>,
    addresses: Vec<Address>,
  }

  impl RemoteTransport for CapturingTransport {
    fn start(&mut self) -> Result<(), TransportError> {
      Ok(())
    }

    fn shutdown(&mut self) -> Result<(), TransportError> {
      Ok(())
    }

    fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), TransportError> {
      self.sent.lock().unwrap().push(envelope);
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
  let _ = association.associate(endpoint, 0);
  let _ = association.handshake_accepted(RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1), 1);
  // Enqueue a system-priority envelope.
  let path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/x").unwrap();
  let envelope = OutboundEnvelope::new(
    path,
    None,
    AnyMessage::new(()),
    OutboundPriority::System,
    RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1),
    CorrelationId::nil(),
  );
  let _ = association.enqueue(envelope);

  let shared = AssociationShared::new(association);
  let sent = Arc::new(Mutex::new(Vec::<OutboundEnvelope>::new()));
  let transport = Arc::new(Mutex::new(CapturingTransport {
    sent:      Arc::clone(&sent),
    addresses: vec![Address::new("local-sys", "127.0.0.1", 2551)],
  }));

  let task_shared = shared.clone();
  let task_transport = Arc::clone(&transport);
  let task = tokio::spawn(async move {
    run_outbound_loop(task_shared, task_transport).await;
  });

  // Allow the outbound loop to drain the queue.
  tokio::time::sleep(Duration::from_millis(20)).await;

  task.abort();
  drop(task.await);

  let sent = sent.lock().unwrap();
  assert_eq!(sent.len(), 1, "outbound loop should have drained one envelope");
}

// ---------------------------------------------------------------------------
// effect_application — verifies that deferred envelopes are NOT lost when the
// handshake completes (regression coverage for the discarded-effects bug).
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
  // Build an association that has been associated and has a deferred envelope
  // queued (because the handshake hasn't completed yet).
  let mut association = sample_association();
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let _ = association.associate(endpoint, 0);
  let _ = association.enqueue(deferred_envelope());
  let _ = association.enqueue(deferred_envelope());
  // Sanity: the envelopes should not yet be drainable from the send queue.
  assert!(association.next_outbound().is_none(), "deferred envelopes must not be drainable before handshake_accepted");

  // Complete the handshake and immediately apply the returned effects in
  // place. This is the contract that production sites
  // (`inbound_dispatch::run_inbound_dispatch`) must honour.
  let effects = association.handshake_accepted(RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1), 1);
  apply_effects_in_place(&mut association, effects);

  // Both deferred envelopes must now be drainable through next_outbound,
  // proving they were re-enqueued into the active send queue rather than
  // silently lost.
  assert!(association.next_outbound().is_some(), "first deferred envelope must be re-enqueued");
  assert!(association.next_outbound().is_some(), "second deferred envelope must be re-enqueued");
  assert!(association.next_outbound().is_none(), "no further envelopes expected");
}

#[test]
fn handshake_timed_out_effects_drop_deferred_envelopes_observably() {
  // Build an association that has been associated and has a deferred envelope
  // queued (because the handshake never completed).
  let mut association = sample_association();
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let _ = association.associate(endpoint, 0);
  let _ = association.enqueue(deferred_envelope());

  // Trigger the timeout transition and apply effects in place. This is the
  // contract that `handshake_driver::HandshakeDriver` must honour.
  let effects = association.handshake_timed_out(0, None);
  apply_effects_in_place(&mut association, effects);

  // The state must now be Gated, and the send queue must be empty (the
  // deferred envelopes were intentionally discarded by the timeout path).
  assert!(association.state().is_gated(), "handshake_timed_out should have moved the association to Gated");
  assert!(association.next_outbound().is_none(), "Gated state must not surface envelopes from next_outbound");
}

#[test]
fn handshake_accepted_with_no_deferred_envelopes_is_a_noop() {
  // Regression coverage: even when there is nothing to flush, applying the
  // effects must not panic and must not produce phantom envelopes.
  let mut association = sample_association();
  let endpoint = TransportEndpoint::new("remote-sys@10.0.0.1:2552");
  let _ = association.associate(endpoint, 0);

  let effects = association.handshake_accepted(RemoteNodeId::new("remote-sys", "10.0.0.1", Some(2552), 1), 1);
  apply_effects_in_place(&mut association, effects);

  assert!(association.next_outbound().is_none());
}
