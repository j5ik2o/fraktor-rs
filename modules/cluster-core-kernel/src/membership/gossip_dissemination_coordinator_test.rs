use alloc::{string::ToString, vec::Vec};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{
  DataCenter, GossipDisseminationCoordinator, GossipEvent, GossipOutbound, GossipState, GossipTransportHandoff,
  MembershipDelta, MembershipTable, MembershipVersion, NodeRecord, NodeStatus,
};

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

#[test]
fn diffusing_reaches_confirmed_after_all_peers_ack() {
  let mut table = MembershipTable::new(3);
  let delta = table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["member".to_string()])
    .expect("join succeeds");
  table.drain_events();

  let mut coordinator = GossipDisseminationCoordinator::new(table, Some("n1:4050".to_string()), vec![
    "node-2".to_string(),
    "node-3".to_string(),
  ]);

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
  assert_eq!(events.first(), Some(&GossipEvent::Disseminated { peers: 2, version: MembershipVersion::new(1) }));
  assert_eq!(events.last(), Some(&GossipEvent::Confirmed { version: MembershipVersion::new(1) }));
  let seen_events = events
    .iter()
    .filter_map(|event| {
      if let GossipEvent::SeenChanged { seen_by, version, .. } = event {
        Some((seen_by.clone(), *version))
      } else {
        None
      }
    })
    .collect::<Vec<_>>();
  assert_eq!(seen_events.len(), 3);
  assert_eq!(seen_events[0], (vec!["n1:4050".to_string()], MembershipVersion::new(1)));
  assert_eq!(seen_events[1], (vec!["n1:4050".to_string(), "node-2".to_string()], MembershipVersion::new(1)));
  assert_eq!(
    seen_events[2],
    (vec!["n1:4050".to_string(), "node-2".to_string(), "node-3".to_string()], MembershipVersion::new(1))
  );
}

#[test]
fn conflict_moves_engine_to_reconciling_and_emits_event() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["member".to_string()])
    .expect("join succeeds");
  table.drain_events();

  let mut coordinator = GossipDisseminationCoordinator::new(table, None, vec!["node-2".to_string()]);

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

  let mut coordinator = GossipDisseminationCoordinator::new(table, None, vec!["node-2".to_string()]);

  coordinator.request_reconcile("node-2", MembershipVersion::new(0), MembershipVersion::new(1));

  assert_eq!(coordinator.state(), GossipState::Reconciling);
  let events = coordinator.drain_events();
  assert_eq!(events, vec![GossipEvent::ReconcilingRequested {
    peer:          "node-2".to_string(),
    local_version: MembershipVersion::new(1),
  }],);
}

#[test]
fn apply_incoming_marks_local_authority_as_seen() {
  let table = MembershipTable::new(3);
  let mut coordinator =
    GossipDisseminationCoordinator::new(table, Some("n1:4050".to_string()), vec!["node-2".to_string()]);

  let incoming_delta =
    MembershipDelta::new(MembershipVersion::zero(), MembershipVersion::new(1), vec![NodeRecord::new(
      "node-2".to_string(),
      "node-2".to_string(),
      NodeStatus::Up,
      MembershipVersion::new(1),
      "1.0.0".to_string(),
      vec!["member".to_string()],
    )]);

  coordinator.apply_incoming(&incoming_delta, "node-2");

  assert_eq!(coordinator.seen_by(), vec!["n1:4050".to_string(), "node-2".to_string()]);

  let seen_events = coordinator
    .drain_events()
    .into_iter()
    .filter_map(|event| {
      if let GossipEvent::SeenChanged { seen_by, version, .. } = event { Some((seen_by, version)) } else { None }
    })
    .collect::<Vec<_>>();
  assert_eq!(seen_events, vec![(vec!["n1:4050".to_string(), "node-2".to_string()], MembershipVersion::new(1))]);
}

#[test]
fn apply_incoming_uses_same_vector_clock_convergence_as_handle_ack() {
  let mut table = MembershipTable::new(3);
  let delta = table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["member".to_string()])
    .expect("join succeeds");
  table.drain_events();

  let mut coordinator =
    GossipDisseminationCoordinator::new(table, None, vec!["node-2".to_string(), "node-3".to_string()]);
  let _ = coordinator.disseminate(&delta);

  coordinator.apply_incoming(&delta, "node-2");
  assert_eq!(coordinator.state(), GossipState::Diffusing);

  let state_after = coordinator.handle_ack("node-3");
  assert_eq!(state_after, Some(GossipState::Confirmed));
  assert_eq!(coordinator.state(), GossipState::Confirmed);
}

#[test]
fn seen_by_tracks_latest_acknowledgements() {
  let mut table = MembershipTable::new(3);
  let delta = table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["member".to_string()])
    .expect("join succeeds");
  table.drain_events();
  let mut coordinator = GossipDisseminationCoordinator::new(table, Some("n1:4050".to_string()), vec![
    "node-2".to_string(),
    "node-3".to_string(),
  ]);

  let _ = coordinator.disseminate(&delta);
  let _ = coordinator.handle_ack("node-3");

  assert_eq!(coordinator.seen_by(), vec!["n1:4050".to_string(), "node-3".to_string()]);

  let _ = coordinator.handle_ack("node-2");
  assert_eq!(coordinator.seen_by(), vec!["n1:4050".to_string(), "node-2".to_string(), "node-3".to_string()]);
}

#[test]
fn seen_digest_projects_acknowledged_authorities_to_member_identities() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-b", 11);
  let mut table = MembershipTable::new(3);
  table
    .try_join_with_identity("node-a".to_string(), local.clone(), DataCenter::new("dc-a"), "1.0.0".to_string(), vec![
      "member".to_string(),
    ])
    .expect("local joins");
  table
    .try_join_with_identity("node-b".to_string(), peer.clone(), DataCenter::new("dc-a"), "1.0.0".to_string(), vec![
      "member".to_string(),
    ])
    .expect("peer joins");
  table.drain_events();
  let current = table.version();
  let local_authority = local.address().to_string();
  let peer_authority = peer.address().to_string();
  let mut coordinator = GossipDisseminationCoordinator::new(table, Some(local_authority), vec![peer_authority.clone()]);
  let delta = MembershipDelta::new(current, current.next(), Vec::new());

  let _ = coordinator.disseminate(&delta);
  let _ = coordinator.handle_ack(peer_authority.as_str());

  let digest = coordinator.seen_digest();
  assert_eq!(digest.observed_version(&local), Some(current.next()));
  assert_eq!(digest.observed_version(&peer), Some(current.next()));
  assert!(digest.has_seen_all(&[local, peer], current.next()));
}

#[test]
fn seen_digest_does_not_promote_old_seen_peer_to_new_incoming_version() {
  let local = unique_address("node-a", 10);
  let peer_b = unique_address("node-b", 11);
  let peer_c = unique_address("node-c", 12);
  let mut table = MembershipTable::new(3);
  table
    .try_join_with_identity("node-a".to_string(), local.clone(), DataCenter::new("dc-a"), "1.0.0".to_string(), vec![
      "member".to_string(),
    ])
    .expect("local joins");
  table
    .try_join_with_identity("node-b".to_string(), peer_b.clone(), DataCenter::new("dc-a"), "1.0.0".to_string(), vec![
      "member".to_string(),
    ])
    .expect("peer b joins");
  table
    .try_join_with_identity("node-c".to_string(), peer_c.clone(), DataCenter::new("dc-a"), "1.0.0".to_string(), vec![
      "member".to_string(),
    ])
    .expect("peer c joins");
  table.drain_events();
  let version = table.version();
  let local_authority = local.address().to_string();
  let peer_b_authority = peer_b.address().to_string();
  let peer_c_authority = peer_c.address().to_string();
  let mut coordinator = GossipDisseminationCoordinator::new(table, Some(local_authority), vec![
    peer_b_authority.clone(),
    peer_c_authority.clone(),
  ]);
  let first_delta = MembershipDelta::new(version, version.next(), Vec::new());
  let second_delta = MembershipDelta::new(version.next(), version.next().next(), Vec::new());

  let _ = coordinator.disseminate(&first_delta);
  let _ = coordinator.handle_ack(peer_b_authority.as_str());
  coordinator.apply_incoming(&second_delta, peer_c_authority.as_str());

  let digest = coordinator.seen_digest();
  assert_eq!(digest.observed_version(&peer_b), Some(version.next()));
  assert_eq!(digest.observed_version(&peer_c), Some(version.next().next()));
  assert!(!digest.has_seen_all(&[local, peer_b, peer_c], version.next().next()));
}

#[test]
fn apply_incoming_marks_endpoint_form_peer_authority_as_seen() {
  let local = unique_address("node-a", 10);
  let peer = unique_address("node-b", 11);
  let mut table = MembershipTable::new(3);
  table
    .try_join_with_identity("node-a".to_string(), local.clone(), DataCenter::new("dc-a"), "1.0.0".to_string(), vec![
      "member".to_string(),
    ])
    .expect("local joins");
  table
    .try_join_with_identity("node-b".to_string(), peer.clone(), DataCenter::new("dc-a"), "1.0.0".to_string(), vec![
      "member".to_string(),
    ])
    .expect("peer joins");
  table.drain_events();
  let current = table.version();
  let mut coordinator =
    GossipDisseminationCoordinator::new(table, Some(local.address().to_string()), vec![peer.address().to_string()]);
  let incoming_delta = MembershipDelta::new(current, current.next(), Vec::new());

  coordinator.apply_incoming(&incoming_delta, GossipTransportHandoff::endpoint_for_identity(&peer).as_str());

  assert_eq!(coordinator.seen_digest().observed_version(&peer), Some(current.next()));
}

#[test]
fn disseminate_marks_self_as_seen_in_single_node_cluster() {
  let mut table = MembershipTable::new(3);
  let delta = table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["member".to_string()])
    .expect("join succeeds");
  table.drain_events();

  let mut coordinator = GossipDisseminationCoordinator::new(table, Some("n1:4050".to_string()), Vec::new());

  let outbound = coordinator.disseminate(&delta);
  assert!(outbound.is_empty());
  assert_eq!(coordinator.state(), GossipState::Confirmed);
  assert_eq!(coordinator.seen_by(), vec!["n1:4050".to_string()]);

  let events = coordinator.drain_events();
  let seen_events = events
    .iter()
    .filter_map(|event| {
      if let GossipEvent::SeenChanged { seen_by, version, .. } = event {
        Some((seen_by.clone(), *version))
      } else {
        None
      }
    })
    .collect::<Vec<_>>();
  assert_eq!(seen_events, vec![(vec!["n1:4050".to_string()], MembershipVersion::new(1))]);
  assert_eq!(events.last(), Some(&GossipEvent::Confirmed { version: MembershipVersion::new(1) }));
}
