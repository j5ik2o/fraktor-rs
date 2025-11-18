#![cfg(test)]

use super::{AssociationState, EndpointManager, QuarantineReason, RemoteNodeId};
use crate::transport::{LoopbackTransport, RemoteTransport, TransportBind, TransportEndpoint};

#[test]
fn handshake_transitions_and_flushes_deferred_messages() {
  let manager = EndpointManager::new();
  let authority = "node-1";

  assert!(matches!(manager.state(authority), AssociationState::Unassociated));
  manager.start_association(authority, 1);
  assert!(matches!(manager.state(authority), AssociationState::Associating { attempt: 1 }));

  manager.defer_message(authority, b"a".to_vec());
  manager.defer_message(authority, b"b".to_vec());

  let remote = RemoteNodeId::new("sys", "host", Some(2552), 42);
  let flushed = manager.complete_handshake(authority, remote.clone(), 2);
  assert_eq!(flushed, vec![b"a".to_vec(), b"b".to_vec()]);
  assert!(matches!(manager.state(authority), AssociationState::Connected { remote: r } if r.uid() == 42));
}

#[test]
fn association_attempt_counter_increments() {
  let manager = EndpointManager::new();
  let authority = "node-2";
  manager.start_association(authority, 0);
  let AssociationState::Associating { attempt } = manager.start_association(authority, 5) else {
    panic!("expected associating state");
  };
  assert_eq!(attempt, 2);
}

#[test]
fn quarantine_transitions_track_reason_and_time() {
  let manager = EndpointManager::new();
  let authority = "node-3";
  manager.defer_message(authority, b"payload".to_vec());
  manager.set_quarantine(authority, QuarantineReason::UidMismatch, 50, None);
  match manager.state(authority) {
    | AssociationState::Quarantined { reason, since, deadline } => {
      assert_eq!(reason, "uid mismatch");
      assert_eq!(since, 50);
      assert!(deadline.is_none());
    },
    | state => panic!("unexpected state {state:?}"),
  }

  let remote = RemoteNodeId::new("sys", "host", Some(1024), 7);
  manager.defer_message(authority, b"fresh".to_vec());
  let flushed = manager.manual_override_to_connected(authority, remote.clone(), 75);
  assert_eq!(flushed, vec![b"fresh".to_vec()]);
  assert!(matches!(manager.state(authority), AssociationState::Connected { remote: r } if r.uid() == 7));
  let snapshot = manager.snapshots().into_iter().find(|snap| snap.authority() == authority).unwrap();
  assert_eq!(snapshot.last_change(), 75);
}

#[test]
fn loopback_roundtrip_after_handshake_flushes_deferred_payloads() {
  use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

  let manager = EndpointManager::new();
  let authority = "loopback";
  let transport = LoopbackTransport::new();
  let handle = <LoopbackTransport as RemoteTransport<NoStdToolbox>>::spawn_listener(
    &transport,
    &TransportBind::new(authority),
  )
  .expect("listener");
  let channel = <LoopbackTransport as RemoteTransport<NoStdToolbox>>::open_channel(
    &transport,
    &TransportEndpoint::new(authority),
  )
  .expect("channel");

  manager.start_association(authority, 1);
  manager.defer_message(authority, b"hello".to_vec());
  manager.defer_message(authority, b"world".to_vec());
  let remote = RemoteNodeId::new("sys", "host", Some(2552), 100);
  let deferred = manager.complete_handshake(authority, remote, 10);
  for payload in deferred {
    <LoopbackTransport as RemoteTransport<NoStdToolbox>>::send(&transport, &channel, &payload)
      .expect("send");
  }
  let frames = handle.take_frames();
  assert_eq!(frames.len(), 2);
  assert_eq!(&frames[0][4..], b"hello");
  assert_eq!(&frames[1][4..], b"world");
}
