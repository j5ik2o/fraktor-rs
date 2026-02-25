use alloc::string::ToString;

use crate::core::membership::{
  GossipDisseminationCoordinator, GossipEvent, GossipOutbound, GossipState, MembershipDelta, MembershipTable,
  MembershipVersion, NodeRecord, NodeStatus,
};

#[test]
fn diffusing_reaches_confirmed_after_all_peers_ack() {
  let mut table = MembershipTable::new(3);
  let delta = table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["member".to_string()])
    .expect("join succeeds");
  table.drain_events();

  let mut coordinator = GossipDisseminationCoordinator::new(table, vec!["node-2".to_string(), "node-3".to_string()]);

  let outbound = coordinator.disseminate(&delta);
  assert_eq!(outbound.len(), 2);
  assert!(outbound.contains(&GossipOutbound::new("node-2".to_string(), delta.clone())));
  assert!(outbound.contains(&GossipOutbound::new("node-3".to_string(), delta.clone())));
  assert_eq!(coordinator.state(), GossipState::Diffusing);

  assert!(coordinator.handle_ack("node-2").is_none());
  let state_after = coordinator.handle_ack("node-3");
  assert_eq!(state_after, Some(GossipState::Confirmed));
  assert_eq!(coordinator.state(), GossipState::Confirmed);

  let events = coordinator.drain_events();
  assert_eq!(events, vec![
    GossipEvent::Disseminated { peers: 2, version: MembershipVersion::new(1) },
    GossipEvent::Confirmed { version: MembershipVersion::new(1) },
  ],);
}

#[test]
fn conflict_moves_engine_to_reconciling_and_emits_event() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["member".to_string()])
    .expect("join succeeds");
  table.drain_events();

  let mut coordinator = GossipDisseminationCoordinator::new(table, vec!["node-2".to_string()]);

  let conflict_delta =
    MembershipDelta::new(MembershipVersion::zero(), MembershipVersion::zero(), vec![NodeRecord::new(
      "other".to_string(),
      "n1:4050".to_string(),
      NodeStatus::Up,
      MembershipVersion::zero(),
      "1.0.0".to_string(),
      vec!["member".to_string()],
    )]);

  coordinator.apply_incoming(&conflict_delta, "node-2");

  assert_eq!(coordinator.state(), GossipState::Reconciling);
  let events = coordinator.drain_events();
  assert_eq!(events, vec![GossipEvent::ConflictDetected {
    peer:           "node-2".to_string(),
    local_version:  MembershipVersion::new(1),
    remote_version: MembershipVersion::zero(),
  }],);
}

#[test]
fn missing_range_request_enters_reconciling() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["member".to_string()])
    .expect("join succeeds");
  table.drain_events();

  let mut coordinator = GossipDisseminationCoordinator::new(table, vec!["node-2".to_string()]);

  coordinator.request_reconcile("node-2", MembershipVersion::new(0), MembershipVersion::new(1));

  assert_eq!(coordinator.state(), GossipState::Reconciling);
  let events = coordinator.drain_events();
  assert_eq!(events, vec![GossipEvent::ReconcilingRequested {
    peer:          "node-2".to_string(),
    local_version: MembershipVersion::new(1),
  }],);
}
