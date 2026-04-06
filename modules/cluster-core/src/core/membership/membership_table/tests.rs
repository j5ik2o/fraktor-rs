use alloc::{string::ToString, vec};

use super::MembershipTable;
use crate::core::membership::{MembershipError, MembershipEvent, MembershipVersion, NodeStatus};

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
fn status_update_does_not_change_age_ordering() {
  let mut table = MembershipTable::new(3);
  table
    .try_join("node-1".to_string(), "n1:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("first join succeeds");
  table
    .try_join("node-2".to_string(), "n2:4050".to_string(), "1.0.0".to_string(), vec!["backend".to_string()])
    .expect("second join succeeds");

  let _ = table.mark_up("n1:4050").expect("mark up succeeds");

  let older = table.record("n1:4050").expect("older record");
  let newer = table.record("n2:4050").expect("newer record");

  assert!(older.version > newer.version);
  assert!(older.is_older_than(newer));
  assert!(!newer.is_older_than(older));
}
