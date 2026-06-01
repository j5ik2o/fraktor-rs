use alloc::{collections::BTreeMap, string::ToString, vec, vec::Vec};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::membership::{
  DataCenter, GossipSeenDigest, GossipStateModel, GossipStateSnapshot, GossipTombstone, GossipTombstoneSet,
  MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus, ReachabilityMatrix, ReachabilityRecord,
  ReachabilitySnapshot, ReachabilityStatus,
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
fn full_state_merge_prefers_ready_for_shutdown_over_preparing() {
  let identity = unique_address("node-a", 10);
  let preparing = state(2, vec![record(identity.clone(), NodeStatus::PreparingForShutdown, 2)]);
  let ready = state(2, vec![record(identity.clone(), NodeStatus::ReadyForShutdown, 2)]);

  let mut preparing_first = GossipStateModel::new(preparing.clone());
  let preparing_outcome = preparing_first.merge(ready.clone());

  let mut ready_first = GossipStateModel::new(ready);
  let ready_outcome = ready_first.merge(preparing);

  assert_eq!(preparing_first.snapshot(), ready_first.snapshot());
  assert_eq!(preparing_first.snapshot().membership.entries[0].status, NodeStatus::ReadyForShutdown);
  assert_eq!(preparing_outcome.conflicts[0].retained.status, NodeStatus::ReadyForShutdown);
  assert_eq!(ready_outcome.conflicts[0].retained.status, NodeStatus::ReadyForShutdown);
}

#[test]
fn full_state_merge_supersedes_older_active_incarnation_for_same_authority() {
  let older = unique_address("node-a", 10);
  let newer = unique_address("node-a", 11);
  let local = state(2, vec![record(older.clone(), NodeStatus::Up, 2)]);
  let remote = state(3, vec![record(newer.clone(), NodeStatus::Up, 3)]);

  let mut model = GossipStateModel::new(local);
  let outcome = model.merge(remote);

  let entries = &model.snapshot().membership.entries;
  assert_eq!(entries.len(), 2);
  assert!(entries.iter().any(|record| record.unique_address == newer && record.status == NodeStatus::Up));
  assert!(entries.iter().any(|record| record.unique_address == older && record.status == NodeStatus::Dead));
  assert_eq!(outcome.stale_records_suppressed.len(), 1);
  assert_eq!(
    model.snapshot().tombstones.get(&older).expect("older incarnation should be tombstoned").version,
    MembershipVersion::new(3)
  );
}

#[test]
fn full_state_merge_supersedes_older_active_incarnation_with_newer_leaving_incarnation() {
  let older = unique_address("node-a", 10);
  let newer = unique_address("node-a", 11);
  let local = state(2, vec![record(older.clone(), NodeStatus::Up, 2)]);
  let remote = state(3, vec![record(newer.clone(), NodeStatus::Leaving, 3)]);

  let mut model = GossipStateModel::new(local);
  let outcome = model.merge(remote);

  let entries = &model.snapshot().membership.entries;
  assert_eq!(entries.len(), 2);
  assert!(entries.iter().any(|record| record.unique_address == newer && record.status == NodeStatus::Leaving));
  assert!(entries.iter().any(|record| record.unique_address == older && record.status == NodeStatus::Dead));
  assert_eq!(outcome.stale_records_suppressed.len(), 1);
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
  assert!(model.snapshot().membership.entries.is_empty());

  let rebuilt = GossipStateModel::new(model.snapshot().clone());
  assert!(rebuilt.snapshot().tombstones.get(&identity).is_none());
}

#[test]
fn tombstone_retention_prunes_reachability_rows_for_retained_member() {
  let member = unique_address("node-a", 10);
  let observer = unique_address("node-b", 11);
  let subject = unique_address("node-c", 12);
  let mut observer_versions = BTreeMap::new();
  observer_versions.insert(observer.clone(), 3);
  observer_versions.insert(member.clone(), 3);
  let local = GossipStateSnapshot::new(
    MembershipSnapshot::new_with_reachability(
      MembershipVersion::new(3),
      vec![record(member.clone(), NodeStatus::Dead, 3), record(observer.clone(), NodeStatus::Up, 1)],
      ReachabilitySnapshot::new(
        vec![
          ReachabilityRecord {
            observer: observer.clone(),
            subject:  member.clone(),
            status:   ReachabilityStatus::Unreachable,
            version:  3,
          },
          ReachabilityRecord {
            observer: member.clone(),
            subject:  subject.clone(),
            status:   ReachabilityStatus::Unreachable,
            version:  3,
          },
        ],
        observer_versions,
      ),
    ),
    GossipTombstoneSet::new(),
  );
  let mut model = GossipStateModel::new(local);

  let pruned = model.prune_retained_tombstones(MembershipVersion::new(3));

  assert_eq!(pruned.pruned.len(), 1);
  assert!(model.snapshot().membership.reachability.records.is_empty());
  assert_eq!(model.snapshot().membership.reachability.observer_versions.get(&observer), Some(&3));
  assert!(!model.snapshot().membership.reachability.observer_versions.contains_key(&member));
  assert_eq!(model.snapshot().membership.reachability.aggregate_status(&member), ReachabilityStatus::Reachable);
}

#[test]
fn seen_digest_tracks_peer_observed_versions_and_convergence() {
  let peer_a = unique_address("node-a", 10);
  let peer_b = unique_address("node-b", 11);
  let mut model = GossipStateModel::new(state(3, Vec::new()));

  assert!(model.mark_seen(peer_a.clone(), MembershipVersion::new(3)));
  assert!(!model.mark_seen(peer_a.clone(), MembershipVersion::new(2)));
  assert!(!model.has_seen_all(&[peer_a.clone(), peer_b.clone()], MembershipVersion::new(3)));

  assert!(model.mark_seen(peer_b.clone(), MembershipVersion::new(3)));

  assert_eq!(model.seen_digest().observed_version(&peer_a), Some(MembershipVersion::new(3)));
  assert!(model.has_seen_all(&[peer_a, peer_b], MembershipVersion::new(3)));
}

#[test]
fn full_state_merge_merges_seen_digest() {
  let peer = unique_address("node-a", 10);
  let local = state(1, Vec::new());
  let mut remote_digest = GossipSeenDigest::new();
  remote_digest.mark_seen(peer.clone(), MembershipVersion::new(4));
  let remote = GossipStateSnapshot::new_with_seen_digest(
    MembershipSnapshot::new(MembershipVersion::new(4), Vec::new()),
    GossipTombstoneSet::new(),
    remote_digest,
  );

  let mut model = GossipStateModel::new(local);
  model.merge(remote);

  assert_eq!(model.seen_digest().observed_version(&peer), Some(MembershipVersion::new(4)));
}

#[test]
fn full_state_merge_prunes_stale_reachability_record_from_newer_reachable_row() {
  let observer = unique_address("node-a", 10);
  let subject = unique_address("node-b", 11);
  let mut local_reachability = ReachabilityMatrix::new();
  local_reachability.unreachable(observer.clone(), subject.clone());
  let local = GossipStateSnapshot::new(
    MembershipSnapshot::new_with_reachability(
      MembershipVersion::new(1),
      vec![record(observer.clone(), NodeStatus::Up, 1), record(subject.clone(), NodeStatus::Up, 1)],
      local_reachability.snapshot(),
    ),
    GossipTombstoneSet::new(),
  );
  let mut remote_reachability = ReachabilityMatrix::new();
  remote_reachability.unreachable(observer.clone(), subject.clone());
  remote_reachability.reachable(observer.clone(), subject.clone());
  let remote = GossipStateSnapshot::new(
    MembershipSnapshot::new_with_reachability(
      MembershipVersion::new(2),
      vec![record(observer.clone(), NodeStatus::Up, 1), record(subject.clone(), NodeStatus::Up, 1)],
      remote_reachability.snapshot(),
    ),
    GossipTombstoneSet::new(),
  );
  let mut model = GossipStateModel::new(local);

  model.merge(remote);

  assert!(model.snapshot().membership.reachability.records.is_empty());
  assert_eq!(model.snapshot().membership.reachability.aggregate_status(&subject), ReachabilityStatus::Reachable);
  assert_eq!(model.snapshot().membership.reachability.observer_versions.get(&observer), Some(&2));
}

#[test]
fn full_state_merge_ignores_remote_reachability_record_older_than_local_row() {
  let observer = unique_address("node-a", 10);
  let subject = unique_address("node-b", 11);
  let mut local_observer_versions = BTreeMap::new();
  local_observer_versions.insert(observer.clone(), 5);
  let local = GossipStateSnapshot::new(
    MembershipSnapshot::new_with_reachability(
      MembershipVersion::new(5),
      vec![record(observer.clone(), NodeStatus::Up, 1), record(subject.clone(), NodeStatus::Up, 1)],
      ReachabilitySnapshot::new(Vec::new(), local_observer_versions),
    ),
    GossipTombstoneSet::new(),
  );
  let mut remote_observer_versions = BTreeMap::new();
  remote_observer_versions.insert(observer.clone(), 4);
  let remote = GossipStateSnapshot::new(
    MembershipSnapshot::new_with_reachability(
      MembershipVersion::new(4),
      vec![record(observer.clone(), NodeStatus::Up, 1), record(subject.clone(), NodeStatus::Up, 1)],
      ReachabilitySnapshot::new(
        vec![ReachabilityRecord {
          observer: observer.clone(),
          subject:  subject.clone(),
          status:   ReachabilityStatus::Unreachable,
          version:  4,
        }],
        remote_observer_versions,
      ),
    ),
    GossipTombstoneSet::new(),
  );
  let mut model = GossipStateModel::new(local);

  model.merge(remote);

  assert!(model.snapshot().membership.reachability.records.is_empty());
  assert_eq!(model.snapshot().membership.reachability.aggregate_status(&subject), ReachabilityStatus::Reachable);
  assert_eq!(model.snapshot().membership.reachability.observer_versions.get(&observer), Some(&5));
}

#[test]
fn full_state_merge_uses_stronger_reachability_status_for_equal_versions() {
  let observer = unique_address("node-a", 10);
  let subject = unique_address("node-b", 11);
  let mut observer_versions = BTreeMap::new();
  observer_versions.insert(observer.clone(), 5);
  let unreachable = GossipStateSnapshot::new(
    MembershipSnapshot::new_with_reachability(
      MembershipVersion::new(5),
      vec![record(observer.clone(), NodeStatus::Up, 1), record(subject.clone(), NodeStatus::Up, 1)],
      ReachabilitySnapshot::new(
        vec![ReachabilityRecord {
          observer: observer.clone(),
          subject:  subject.clone(),
          status:   ReachabilityStatus::Unreachable,
          version:  5,
        }],
        observer_versions.clone(),
      ),
    ),
    GossipTombstoneSet::new(),
  );
  let terminated = GossipStateSnapshot::new(
    MembershipSnapshot::new_with_reachability(
      MembershipVersion::new(5),
      vec![record(observer.clone(), NodeStatus::Up, 1), record(subject.clone(), NodeStatus::Up, 1)],
      ReachabilitySnapshot::new(
        vec![ReachabilityRecord {
          observer: observer.clone(),
          subject:  subject.clone(),
          status:   ReachabilityStatus::Terminated,
          version:  5,
        }],
        observer_versions,
      ),
    ),
    GossipTombstoneSet::new(),
  );

  let mut unreachable_first = GossipStateModel::new(unreachable.clone());
  unreachable_first.merge(terminated.clone());
  let mut terminated_first = GossipStateModel::new(terminated);
  terminated_first.merge(unreachable);

  assert_eq!(unreachable_first.snapshot(), terminated_first.snapshot());
  assert_eq!(unreachable_first.snapshot().membership.reachability.records[0].status, ReachabilityStatus::Terminated);
  assert_eq!(
    unreachable_first.snapshot().membership.reachability.aggregate_status(&subject),
    ReachabilityStatus::Terminated
  );
}

#[test]
fn tombstone_prune_waits_until_seen_by_all_active_peers() {
  let member = unique_address("node-a", 10);
  let peer_a = unique_address("node-b", 11);
  let peer_b = unique_address("node-c", 12);
  let local = state(3, vec![record(member.clone(), NodeStatus::Dead, 3)]);
  let mut model = GossipStateModel::new(local);

  model.mark_seen(peer_a.clone(), MembershipVersion::new(3));
  let early = model.prune_tombstones_when_seen_by_all(&[peer_a.clone(), peer_b.clone()], MembershipVersion::new(3));
  assert!(early.pruned.is_empty());
  assert!(model.snapshot().tombstones.get(&member).is_some());

  model.mark_seen(peer_b.clone(), MembershipVersion::new(3));
  let pruned = model.prune_tombstones_when_seen_by_all(&[peer_a, peer_b], MembershipVersion::new(3));
  assert_eq!(pruned.pruned.len(), 1);
  assert!(model.snapshot().tombstones.get(&member).is_none());
  assert!(model.snapshot().membership.entries.is_empty());
}
