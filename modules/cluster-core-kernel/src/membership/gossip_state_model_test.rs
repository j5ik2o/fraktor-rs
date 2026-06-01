use alloc::{string::ToString, vec, vec::Vec};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{
  DataCenter, GossipStateModel, GossipStateSnapshot, GossipTombstone, GossipTombstoneSet, MembershipSnapshot,
  MembershipVersion, NodeRecord, NodeStatus,
};

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

fn record(identity: UniqueAddress, status: NodeStatus, version: u64) -> NodeRecord {
  NodeRecord::new_with_identity(
    identity,
    DataCenter::new("dc-a"),
    "node-a".to_string(),
    status,
    MembershipVersion::new(version),
    "1.0.0".to_string(),
    vec!["member".to_string()],
  )
}

fn record_with_role(identity: UniqueAddress, status: NodeStatus, version: u64, role: &str) -> NodeRecord {
  NodeRecord::new_with_identity(
    identity,
    DataCenter::new("dc-a"),
    "node-a".to_string(),
    status,
    MembershipVersion::new(version),
    "1.0.0".to_string(),
    vec![role.to_string()],
  )
}

fn state(version: u64, entries: Vec<NodeRecord>) -> GossipStateSnapshot {
  GossipStateSnapshot::new(MembershipSnapshot::new(MembershipVersion::new(version), entries), GossipTombstoneSet::new())
}

#[test]
fn full_state_merge_uses_deterministic_member_precedence() {
  let identity = unique_address("node-a", 10);
  let local = state(1, vec![record(identity.clone(), NodeStatus::WeaklyUp, 1)]);
  let remote = state(2, vec![record(identity.clone(), NodeStatus::Up, 2)]);

  let mut local_first = GossipStateModel::new(local.clone());
  let outcome = local_first.merge(remote.clone());

  let mut remote_first = GossipStateModel::new(remote);
  let reverse_outcome = remote_first.merge(local);

  assert_eq!(local_first.snapshot(), remote_first.snapshot());
  assert_eq!(local_first.snapshot().membership.version, MembershipVersion::new(2));
  assert_eq!(local_first.snapshot().membership.entries[0].status, NodeStatus::Up);
  assert_eq!(outcome.applied_records.len(), 1);
  assert_eq!(reverse_outcome.applied_records.len(), 0);
}

#[test]
fn full_state_merge_reports_remote_wins_conflict() {
  let identity = unique_address("node-a", 10);
  let local = state(2, vec![record(identity.clone(), NodeStatus::WeaklyUp, 2)]);
  let remote = state(2, vec![record(identity.clone(), NodeStatus::Up, 2)]);

  let mut model = GossipStateModel::new(local);
  let outcome = model.merge(remote);

  assert_eq!(model.snapshot().membership.entries[0].status, NodeStatus::Up);
  assert_eq!(outcome.conflicts.len(), 1);
  assert_eq!(outcome.conflicts[0].retained.status, NodeStatus::Up);
  assert_eq!(outcome.conflicts[0].ignored.status, NodeStatus::WeaklyUp);
}

#[test]
fn full_state_merge_uses_stable_tie_breaker_for_equal_version_status_conflict() {
  let identity = unique_address("node-a", 10);
  let left = state(2, vec![record_with_role(identity.clone(), NodeStatus::Up, 2, "backend")]);
  let right = state(2, vec![record_with_role(identity.clone(), NodeStatus::Up, 2, "edge")]);

  let mut left_first = GossipStateModel::new(left.clone());
  let left_outcome = left_first.merge(right.clone());

  let mut right_first = GossipStateModel::new(right);
  let right_outcome = right_first.merge(left);

  assert_eq!(left_first.snapshot(), right_first.snapshot());
  assert_eq!(left_first.snapshot().membership.entries[0].roles, vec!["edge".to_string()]);
  assert_eq!(left_outcome.conflicts.len(), 1);
  assert_eq!(right_outcome.conflicts.len(), 1);
}

#[test]
fn tombstone_suppresses_stale_active_reappearance() {
  let identity = unique_address("node-a", 10);
  let local = state(3, vec![record(identity.clone(), NodeStatus::Removed, 3)]);
  let remote = state(2, vec![record(identity.clone(), NodeStatus::Up, 2)]);

  let mut model = GossipStateModel::new(local);
  let outcome = model.merge(remote);

  assert_eq!(model.snapshot().membership.entries.len(), 1);
  assert_eq!(model.snapshot().membership.entries[0].status, NodeStatus::Removed);
  assert_eq!(outcome.stale_records_suppressed.len(), 1);
  assert_eq!(
    model.snapshot().tombstones.get(&identity).expect("tombstone should exist").version,
    MembershipVersion::new(3)
  );
}

#[test]
fn full_state_merge_reports_remote_tombstone_addition() {
  let identity = unique_address("node-a", 10);
  let local = state(2, vec![record(identity.clone(), NodeStatus::Up, 2)]);
  let remote = state(3, vec![record(identity.clone(), NodeStatus::Dead, 3)]);

  let mut model = GossipStateModel::new(local);
  let outcome = model.merge(remote);

  assert_eq!(model.snapshot().membership.entries[0].status, NodeStatus::Dead);
  assert_eq!(outcome.tombstones_added.len(), 1);
  assert_eq!(outcome.tombstones_added[0].member, identity);
  assert_eq!(outcome.tombstones_added[0].version, MembershipVersion::new(3));
}

#[test]
fn tombstone_only_full_state_suppresses_local_active_record() {
  let identity = unique_address("node-a", 10);
  let local = state(2, vec![record(identity.clone(), NodeStatus::Up, 2)]);
  let mut remote_tombstones = GossipTombstoneSet::new();
  remote_tombstones.insert(GossipTombstone::new(identity.clone(), MembershipVersion::new(3)));
  let remote =
    GossipStateSnapshot::new(MembershipSnapshot::new(MembershipVersion::new(3), Vec::new()), remote_tombstones);

  let mut model = GossipStateModel::new(local);
  let outcome = model.merge(remote);

  assert!(model.snapshot().membership.entries.is_empty());
  assert_eq!(outcome.stale_records_suppressed.len(), 1);
  assert_eq!(outcome.stale_records_suppressed[0].unique_address, identity);
}

#[test]
fn tombstone_only_state_suppresses_stale_non_active_record() {
  let identity = unique_address("node-a", 10);
  let mut local_tombstones = GossipTombstoneSet::new();
  local_tombstones.insert(GossipTombstone::new(identity.clone(), MembershipVersion::new(3)));
  let local =
    GossipStateSnapshot::new(MembershipSnapshot::new(MembershipVersion::new(3), Vec::new()), local_tombstones);
  let remote = state(2, vec![record(identity.clone(), NodeStatus::Exiting, 2)]);

  let mut model = GossipStateModel::new(local);
  let outcome = model.merge(remote);

  assert!(model.snapshot().membership.entries.is_empty());
  assert_eq!(outcome.stale_records_suppressed.len(), 1);
  assert_eq!(outcome.stale_records_suppressed[0].unique_address, identity);
}

#[test]
fn tombstone_retention_prunes_only_converged_versions() {
  let identity = unique_address("node-a", 10);
  let local = state(3, vec![record(identity.clone(), NodeStatus::Dead, 3)]);
  let mut model = GossipStateModel::new(local);

  let early = model.prune_retained_tombstones(MembershipVersion::new(2));
  assert!(early.pruned.is_empty());
  assert!(model.snapshot().tombstones.get(&identity).is_some());

  let retained = model.prune_retained_tombstones(MembershipVersion::new(3));
  assert_eq!(retained.pruned.len(), 1);
  assert!(model.snapshot().tombstones.get(&identity).is_none());
}
