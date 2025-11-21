use alloc::string::ToString;

use crate::core::{
  membership_error::MembershipError,
  membership_event::MembershipEvent,
  membership_version::MembershipVersion,
  node_status::NodeStatus,
};

use super::MembershipTable;

#[test]
fn join_promotes_to_up_and_snapshots_latest_table() {
  let mut table = MembershipTable::new(3);

  let delta = table.try_join("node-1".to_string(), "n1:4050".to_string()).expect("join should succeed");

  assert_eq!(delta.from, MembershipVersion::zero());
  assert_eq!(delta.to, MembershipVersion::new(1));
  assert_eq!(delta.entries.len(), 1);
  let joined = &delta.entries[0];
  assert_eq!(joined.status, NodeStatus::Up);

  let snapshot = table.snapshot();
  assert_eq!(snapshot.version, MembershipVersion::new(1));
  assert_eq!(snapshot.entries.len(), 1);
  assert_eq!(snapshot.entries[0].status, NodeStatus::Up);

  let events = table.drain_events();
  assert_eq!(
    events,
    vec![MembershipEvent::Joined { node_id: "node-1".to_string(), authority: "n1:4050".to_string() }],
  );
}

#[test]
fn joining_with_conflicting_authority_is_rejected() {
  let mut table = MembershipTable::new(3);

  table.try_join("node-1".to_string(), "n1:4050".to_string()).expect("first join succeeds");
  table.drain_events();

  let err = table
    .try_join("node-2".to_string(), "n1:4050".to_string())
    .expect_err("conflict should be rejected");

  assert_eq!(
    err,
    MembershipError::AuthorityConflict {
      authority: "n1:4050".to_string(),
      existing_node_id: "node-1".to_string(),
      requested_node_id: "node-2".to_string(),
    },
  );

  let snapshot = table.snapshot();
  assert_eq!(snapshot.version, MembershipVersion::new(1));
  assert_eq!(snapshot.entries.len(), 1);

  let events = table.drain_events();
  assert_eq!(
    events,
    vec![MembershipEvent::AuthorityConflict {
      authority: "n1:4050".to_string(),
      existing_node_id: "node-1".to_string(),
      requested_node_id: "node-2".to_string(),
    }],
  );
}

#[test]
fn leaving_transitions_to_removed_and_updates_version() {
  let mut table = MembershipTable::new(3);
  table.try_join("node-1".to_string(), "n1:4050".to_string()).expect("join succeeds");
  table.drain_events();

  let delta = table.mark_left("n1:4050").expect("leave should succeed");

  assert_eq!(delta.from, MembershipVersion::new(1));
  assert_eq!(delta.to, MembershipVersion::new(2));
  assert_eq!(delta.entries[0].status, NodeStatus::Removed);

  let snapshot = table.snapshot();
  assert_eq!(snapshot.version, MembershipVersion::new(2));
  assert_eq!(snapshot.entries[0].status, NodeStatus::Removed);

  let events = table.drain_events();
  assert_eq!(events, vec![MembershipEvent::Left { node_id: "node-1".to_string(), authority: "n1:4050".to_string() }]);
}

#[test]
fn heartbeat_miss_marks_unreachable_after_threshold() {
  let mut table = MembershipTable::new(2);
  table.try_join("node-1".to_string(), "n1:4050".to_string()).expect("join succeeds");
  table.drain_events();

  assert!(table.mark_heartbeat_miss("n1:4050").is_none());
  let delta = table
    .mark_heartbeat_miss("n1:4050")
    .expect("second miss should mark unreachable");

  assert_eq!(delta.from, MembershipVersion::new(1));
  assert_eq!(delta.to, MembershipVersion::new(2));
  assert_eq!(delta.entries[0].status, NodeStatus::Unreachable);

  let snapshot = table.snapshot();
  assert_eq!(snapshot.version, MembershipVersion::new(2));
  assert_eq!(snapshot.entries[0].status, NodeStatus::Unreachable);

  let events = table.drain_events();
  assert_eq!(
    events,
    vec![MembershipEvent::MarkedUnreachable { node_id: "node-1".to_string(), authority: "n1:4050".to_string() }],
  );
}
