use alloc::{string::ToString, vec};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use super::MembershipTable;
use crate::membership::{
  DataCenter, MembershipDelta, MembershipError, MembershipEvent, MembershipVersion, NodeRecord, NodeStatus,
};

#[test]
fn join_registers_joining_and_snapshots_latest_table() {
  let mut table = MembershipTable::new(3);

  let delta = table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.2.3".to_string(), vec![
      "backend".to_string(),
      "edge".to_string(),
    ])
    .expect("join should succeed");

  assert_eq!(delta.from, MembershipVersion::zero());
  assert_eq!(delta.to, MembershipVersion::new(1));
  assert_eq!(delta.entries.len(), 1);
  let joined = &delta.entries[0];
  assert_eq!(joined.status, NodeStatus::Joining);
  assert_eq!(joined.join_version, MembershipVersion::new(1));
  assert_eq!(joined.app_version, "1.2.3");
  assert_eq!(joined.roles, vec!["backend".to_string(), "edge".to_string()]);

  let snapshot = table.snapshot();
  assert_eq!(snapshot.version, MembershipVersion::new(1));
  assert_eq!(snapshot.entries.len(), 1);
  assert_eq!(snapshot.entries[0].status, NodeStatus::Joining);

  let events = table.drain_events();
  assert_eq!(events, vec![MembershipEvent::Joined {
    node_id:   "node-1".to_string(),
    authority: "n1:4050".to_string(),
  }],);
}

#[test]
fn joining_with_conflicting_authority_is_rejected() {
  let mut table = MembershipTable::new(3);

  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("first join succeeds");
  table.drain_events();

  let err = table
    .try_join("node-2".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect_err("conflict should be rejected");

  assert_eq!(err, MembershipError::AuthorityConflict {
    authority:         "n1:4050".to_string(),
    existing_node_id:  "node-1".to_string(),
    requested_node_id: "node-2".to_string(),
  },);

  let snapshot = table.snapshot();
  assert_eq!(snapshot.version, MembershipVersion::new(1));
  assert_eq!(snapshot.entries.len(), 1);

  let events = table.drain_events();
  assert_eq!(events, vec![MembershipEvent::AuthorityConflict {
    authority:         "n1:4050".to_string(),
    existing_node_id:  "node-1".to_string(),
    requested_node_id: "node-2".to_string(),
  }],);
}

#[test]
fn join_with_identity_keeps_same_address_different_uid_as_distinct_incarnation() {
  let mut table = MembershipTable::new(3);
  let address = Address::new("cluster", "n1", 4050);
  let first = UniqueAddress::new(address.clone(), 10);
  let second = UniqueAddress::new(address, 11);

  table
    .try_join_with_identity("node-1".to_string(), first.clone(), DataCenter::new("dc-east"), "1.0.0".to_string(), vec![
      "backend".to_string(),
    ])
    .expect("first incarnation joins");
  let delta = table
    .try_join_with_identity(
      "node-1".to_string(),
      second.clone(),
      DataCenter::new("dc-east"),
      "1.0.1".to_string(),
      vec!["backend".to_string()],
    )
    .expect("second incarnation joins");

  assert_eq!(delta.entries[0].unique_address, second);

  let snapshot = table.snapshot();
  assert_eq!(snapshot.entries.len(), 2);
  assert!(snapshot.entries.iter().any(|record| record.unique_address == first && record.status == NodeStatus::Dead));
  assert!(
    snapshot.entries.iter().any(|record| record.unique_address == second && record.status == NodeStatus::Joining)
  );
  assert_eq!(table.record("cluster@n1:4050").expect("record should resolve latest active").unique_address, second);
}

#[test]
fn join_with_identity_rejects_unconfirmed_uid() {
  let mut table = MembershipTable::new(3);
  let identity = UniqueAddress::new(Address::new("cluster", "n1", 4050), 0);

  let err = table
    .try_join_with_identity("node-1".to_string(), identity.clone(), DataCenter::default(), "1.0.0".to_string(), vec![
      "backend".to_string(),
    ])
    .expect_err("unconfirmed uid must be rejected");

  assert_eq!(err, MembershipError::UnconfirmedIdentity { unique_address: identity.to_string() });
  assert!(table.snapshot().entries.is_empty());
}

#[test]
fn join_with_identity_rejects_same_unique_address_node_conflict() {
  let mut table = MembershipTable::new(3);
  let identity = UniqueAddress::new(Address::new("cluster", "n1", 4050), 10);
  table
    .try_join_with_identity(
      "node-1".to_string(),
      identity.clone(),
      DataCenter::new("dc-east"),
      "1.0.0".to_string(),
      vec!["backend".to_string()],
    )
    .expect("first identity join succeeds");

  let err = table
    .try_join_with_identity("node-2".to_string(), identity, DataCenter::new("dc-east"), "1.0.0".to_string(), vec![
      "backend".to_string(),
    ])
    .expect_err("same identity must keep node ownership");

  assert_eq!(err, MembershipError::AuthorityConflict {
    authority:         "cluster@n1:4050".to_string(),
    existing_node_id:  "node-1".to_string(),
    requested_node_id: "node-2".to_string(),
  });
  assert_eq!(table.snapshot().entries.len(), 1);
}

#[test]
fn plain_join_and_gossip_delta_share_unique_address_key() {
  let mut table = MembershipTable::new(3);
  let delta = table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");
  let mut record = delta.entries[0].clone();
  record.status = NodeStatus::WeaklyUp;
  record.version = MembershipVersion::new(2);

  table.apply_delta(MembershipDelta::new(MembershipVersion::new(1), MembershipVersion::new(2), vec![record]));

  let snapshot = table.snapshot();
  assert_eq!(snapshot.entries.len(), 1);
  assert_eq!(table.record("n1:4050").expect("record should exist").status, NodeStatus::WeaklyUp);
}

#[test]
fn join_after_gossip_delta_rejects_same_authority_conflict() {
  let mut origin = MembershipTable::new(3);
  let delta = origin
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");
  let mut receiver = MembershipTable::new(3);
  receiver.apply_delta(delta);

  let err = receiver
    .try_join("node-2".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect_err("same authority should still conflict after gossip");

  assert_eq!(err, MembershipError::AuthorityConflict {
    authority:         "n1:4050".to_string(),
    existing_node_id:  "node-1".to_string(),
    requested_node_id: "node-2".to_string(),
  });
  assert_eq!(receiver.snapshot().entries.len(), 1);
}

#[test]
fn gossip_delta_supersedes_previous_active_incarnation() {
  let mut table = MembershipTable::new(3);
  let address = Address::new("cluster", "n1", 4050);
  let first = UniqueAddress::new(address.clone(), 10);
  let second = UniqueAddress::new(address, 11);
  table
    .try_join_with_identity("node-1".to_string(), first.clone(), DataCenter::new("dc-east"), "1.0.0".to_string(), vec![
      "backend".to_string(),
    ])
    .expect("first incarnation joins");
  table.mark_weakly_up("cluster@n1:4050").expect("weakly up").expect("delta");
  table.mark_up("cluster@n1:4050").expect("up").expect("delta");

  let version = table.version();
  let record = NodeRecord::new_with_identity(
    second.clone(),
    DataCenter::new("dc-east"),
    "node-1".to_string(),
    NodeStatus::Joining,
    version.next(),
    "1.0.1".to_string(),
    vec!["backend".to_string()],
  );
  table.apply_delta(MembershipDelta::new(version, version.next(), vec![record]));

  let snapshot = table.snapshot();
  assert_eq!(snapshot.entries.len(), 2);
  assert!(snapshot.entries.iter().any(|record| record.unique_address == first && record.status == NodeStatus::Dead));
  assert_eq!(table.record("cluster@n1:4050").expect("record should resolve active incarnation").unique_address, second);
}

#[test]
fn leaving_transitions_from_exiting_to_removed_and_updates_version() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");
  table.drain_events();

  let exiting_delta = table.mark_left("n1:4050").expect("first leave should mark exiting");
  assert_eq!(exiting_delta.from, MembershipVersion::new(1));
  assert_eq!(exiting_delta.to, MembershipVersion::new(2));
  assert_eq!(exiting_delta.entries[0].status, NodeStatus::Exiting);

  let removed_delta = table.mark_left("n1:4050").expect("second leave should mark removed");
  assert_eq!(removed_delta.from, MembershipVersion::new(2));
  assert_eq!(removed_delta.to, MembershipVersion::new(3));
  assert_eq!(removed_delta.entries[0].status, NodeStatus::Removed);

  let snapshot = table.snapshot();
  assert_eq!(snapshot.version, MembershipVersion::new(3));
  assert_eq!(snapshot.entries[0].status, NodeStatus::Removed);

  let events = table.drain_events();
  assert_eq!(events, vec![MembershipEvent::Left { node_id: "node-1".to_string(), authority: "n1:4050".to_string() }]);
}

#[test]
fn heartbeat_miss_marks_suspect_after_threshold() {
  let mut table = MembershipTable::new(2);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");
  table.drain_events();

  assert!(table.mark_heartbeat_miss("n1:4050").is_none());
  let delta = table.mark_heartbeat_miss("n1:4050").expect("second miss should mark suspect");

  assert_eq!(delta.from, MembershipVersion::new(1));
  assert_eq!(delta.to, MembershipVersion::new(2));
  assert_eq!(delta.entries[0].status, NodeStatus::Suspect);

  let snapshot = table.snapshot();
  assert_eq!(snapshot.version, MembershipVersion::new(2));
  assert_eq!(snapshot.entries[0].status, NodeStatus::Suspect);

  let events = table.drain_events();
  assert_eq!(events, vec![MembershipEvent::MarkedSuspect {
    node_id:   "node-1".to_string(),
    authority: "n1:4050".to_string(),
  }],);
}

#[test]
fn joining_member_transitions_through_weakly_up_before_up() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");

  let weakly_up_delta = table.mark_weakly_up("n1:4050").expect("weakly up succeeds").expect("delta");
  assert_eq!(weakly_up_delta.entries[0].status, NodeStatus::WeaklyUp);
  assert!(weakly_up_delta.entries[0].status.is_provisional());

  let up_delta = table.mark_up("n1:4050").expect("mark up succeeds").expect("delta");
  assert_eq!(up_delta.entries[0].status, NodeStatus::Up);
}

#[test]
fn mark_up_from_joining_is_rejected_until_weakly_up() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");

  let err = table.mark_up("n1:4050").expect_err("joining cannot skip weakly up");

  assert_eq!(err, MembershipError::InvalidTransition {
    authority: "n1:4050".to_string(),
    from:      NodeStatus::Joining,
    to:        NodeStatus::Up,
  });
}

#[test]
fn weakly_up_can_leave_or_be_marked_dead() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");
  table.mark_weakly_up("n1:4050").expect("weakly up succeeds");

  let exiting_delta = table.mark_left("n1:4050").expect("weakly up can leave");
  assert_eq!(exiting_delta.entries[0].status, NodeStatus::Exiting);
  let removed_delta = table.mark_left("n1:4050").expect("weakly up leave can complete removal");
  assert_eq!(removed_delta.entries[0].status, NodeStatus::Removed);

  let mut table = MembershipTable::new(3);
  table
    .try_join("node-2".to_string(), "n2:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");
  table.mark_weakly_up("n2:4050").expect("weakly up succeeds");
  let dead_delta = table.mark_dead("n2:4050").expect("weakly up can be downed").expect("delta");
  assert_eq!(dead_delta.entries[0].status, NodeStatus::Dead);
}

#[test]
fn active_member_can_be_marked_dead() {
  for (status, authority, node_id) in [
    (NodeStatus::Joining, "n1:4050", "node-joining"),
    (NodeStatus::WeaklyUp, "n2:4050", "node-weakly-up"),
    (NodeStatus::Up, "n3:4050", "node-up"),
    (NodeStatus::Suspect, "n4:4050", "node-suspect"),
  ] {
    let mut table = MembershipTable::new(3);
    table
      .try_join(node_id.to_string(), authority.to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
      .expect("join succeeds");
    match status {
      | NodeStatus::Joining => {},
      | NodeStatus::WeaklyUp => {
        table.mark_weakly_up(authority).expect("weakly up succeeds");
      },
      | NodeStatus::Up => {
        table.mark_weakly_up(authority).expect("weakly up succeeds");
        table.mark_up(authority).expect("up succeeds");
      },
      | NodeStatus::Suspect => {
        table.mark_suspect(authority).expect("suspect succeeds");
      },
      | _ => unreachable!("test covers active statuses only"),
    }

    let dead_delta = table.mark_dead(authority).expect("active member can be downed").expect("delta");

    assert_eq!(dead_delta.entries[0].status, NodeStatus::Dead);
  }
}

#[test]
fn heartbeat_miss_is_ignored_for_exiting_member() {
  let mut table = MembershipTable::new(1);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");
  table.drain_events();
  table.mark_left("n1:4050").expect("leave to exiting");

  assert!(table.mark_heartbeat_miss("n1:4050").is_none());
  let status = table.record("n1:4050").expect("record").status;
  assert_eq!(status, NodeStatus::Exiting);
}

#[test]
fn rejoin_from_removed_updates_metadata() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");
  table.drain_events();
  table.mark_left("n1:4050").expect("first leave should mark exiting");
  table.mark_left("n1:4050").expect("second leave should mark removed");

  let delta = table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "2.0.0".to_string(), vec![
      "frontend".to_string(),
      "canary".to_string(),
    ])
    .expect("rejoin from removed should succeed");

  assert_eq!(delta.entries[0].status, NodeStatus::Joining);
  assert_eq!(delta.entries[0].join_version, MembershipVersion::new(4));
  assert_eq!(delta.entries[0].app_version, "2.0.0");
  assert_eq!(delta.entries[0].roles, vec!["frontend".to_string(), "canary".to_string()]);

  let record = table.record("n1:4050").expect("record should exist after rejoin");
  assert_eq!(record.join_version, MembershipVersion::new(4));
  assert_eq!(record.app_version, "2.0.0");
  assert_eq!(record.roles, vec!["frontend".to_string(), "canary".to_string()]);
}

#[test]
fn rejoin_from_dead_updates_metadata() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("join succeeds");
  table.drain_events();
  table.mark_suspect("n1:4050").expect("mark suspect should succeed");
  table.mark_dead("n1:4050").expect("mark dead should succeed");

  let delta = table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "2.0.0".to_string(), vec![
      "frontend".to_string(),
      "canary".to_string(),
    ])
    .expect("rejoin from dead should succeed");

  assert_eq!(delta.entries[0].status, NodeStatus::Joining);
  assert_eq!(delta.entries[0].join_version, MembershipVersion::new(4));
  assert_eq!(delta.entries[0].app_version, "2.0.0");
  assert_eq!(delta.entries[0].roles, vec!["frontend".to_string(), "canary".to_string()]);

  let record = table.record("n1:4050").expect("record should exist after rejoin");
  assert_eq!(record.join_version, MembershipVersion::new(4));
  assert_eq!(record.app_version, "2.0.0");
  assert_eq!(record.roles, vec!["frontend".to_string(), "canary".to_string()]);
}

#[test]
fn rejoin_with_identity_updates_data_center() {
  let mut table = MembershipTable::new(3);
  let identity = UniqueAddress::new(Address::new("cluster", "n1", 4050), 10);
  table
    .try_join_with_identity(
      "node-1".to_string(),
      identity.clone(),
      DataCenter::new("dc-east"),
      "1.0.0".to_string(),
      vec!["backend".to_string()],
    )
    .expect("join succeeds");
  table.mark_suspect("cluster@n1:4050").expect("mark suspect should succeed");
  table.mark_dead("cluster@n1:4050").expect("mark dead should succeed");

  let delta = table
    .try_join_with_identity("node-1".to_string(), identity, DataCenter::new("dc-west"), "2.0.0".to_string(), vec![
      "backend".to_string(),
    ])
    .expect("rejoin succeeds");

  assert_eq!(delta.entries[0].data_center, DataCenter::new("dc-west"));
  assert_eq!(table.record("cluster@n1:4050").expect("record should exist").data_center, DataCenter::new("dc-west"));
}

#[test]
fn status_update_does_not_change_age_ordering() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("first join succeeds");
  table
    .try_join("node-2".to_string(), "n2:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("second join succeeds");

  let _ = table.mark_weakly_up("n1:4050").expect("mark weakly up succeeds");
  let _ = table.mark_up("n1:4050").expect("mark up succeeds");

  let older = table.record("n1:4050").expect("older record");
  let newer = table.record("n2:4050").expect("newer record");

  assert!(older.version > newer.version);
  assert!(older.is_older_than(newer));
  assert!(!newer.is_older_than(older));
}
