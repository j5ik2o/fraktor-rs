#![cfg(test)]

use super::{AssociationState, EndpointManager, RemoteNodeId};

#[test]
fn handshake_transitions_and_flushes_deferred_messages() {
  let manager = EndpointManager::new();
  let authority = "node-1";

  assert!(matches!(manager.state(authority), AssociationState::Unassociated));
  manager.start_association(authority);
  assert!(matches!(manager.state(authority), AssociationState::Associating { attempt: 1 }));

  manager.defer_message(authority, b"a".to_vec());
  manager.defer_message(authority, b"b".to_vec());

  let remote = RemoteNodeId::new("sys", "host", Some(2552), 42);
  let flushed = manager.complete_handshake(authority, remote.clone());
  assert_eq!(flushed, vec![b"a".to_vec(), b"b".to_vec()]);
  assert!(matches!(manager.state(authority), AssociationState::Connected { remote: r } if r.uid() == 42));
}

#[test]
fn association_attempt_counter_increments() {
  let manager = EndpointManager::new();
  let authority = "node-2";
  manager.start_association(authority);
  let AssociationState::Associating { attempt } = manager.start_association(authority) else {
    panic!("expected associating state");
  };
  assert_eq!(attempt, 2);
}
