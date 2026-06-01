//! Full gossip state merge model.

use alloc::{collections::BTreeMap, vec::Vec};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  GossipMergeConflict, GossipMergeOutcome, GossipSeenDigest, GossipStateSnapshot, GossipTombstonePruneOutcome,
  GossipTombstoneSet, MembershipSnapshot, MembershipVersion, NodeRecord, NodeStatus, ReachabilityRecord,
  ReachabilitySnapshot, ReachabilityStatus,
};

#[cfg(test)]
#[path = "gossip_state_model_test.rs"]
mod tests;

/// Merges full gossip state, tombstones, and reachability snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipStateModel {
  snapshot: GossipStateSnapshot,
}

impl GossipStateModel {
  /// Creates a state model and derives tombstones from terminal local records.
  #[must_use]
  pub fn new(mut snapshot: GossipStateSnapshot) -> Self {
    let terminal_tombstones = GossipTombstoneSet::from_records(&snapshot.membership.entries);
    snapshot.tombstones.merge(&terminal_tombstones);
    Self { snapshot }
  }

  /// Returns the current full gossip state snapshot.
  #[must_use]
  pub const fn snapshot(&self) -> &GossipStateSnapshot {
    &self.snapshot
  }

  /// Merges a remote full gossip state into the local model.
  pub fn merge(&mut self, remote: GossipStateSnapshot) -> GossipMergeOutcome {
    let remote = Self::new(remote).snapshot;
    let mut outcome = GossipMergeOutcome::empty();
    outcome.tombstones_added.extend(self.snapshot.tombstones.merge(&remote.tombstones));
    self.snapshot.seen_digest.merge(&remote.seen_digest);

    let mut records = self
      .snapshot
      .membership
      .entries
      .iter()
      .cloned()
      .map(|record| (record.unique_address.clone(), record))
      .collect::<BTreeMap<_, _>>();
    let suppressed_keys = records
      .iter()
      .filter(|(_, record)| self.snapshot.tombstones.suppresses(record))
      .map(|(key, record)| (key.clone(), record.clone()))
      .collect::<Vec<_>>();
    for (key, record) in suppressed_keys {
      if records.remove(&key).is_some() {
        outcome.stale_records_suppressed.push(record);
      }
    }

    for remote_record in remote.membership.entries {
      if self.snapshot.tombstones.suppresses(&remote_record) {
        outcome.stale_records_suppressed.push(remote_record);
        continue;
      }
      if records.values().any(|record| {
        record.unique_address != remote_record.unique_address
          && record.authority == remote_record.authority
          && is_live_incarnation_status(record.status)
          && record.join_version > remote_record.join_version
      }) {
        outcome.stale_records_suppressed.push(remote_record);
        continue;
      }
      let superseded_keys = records
        .iter()
        .filter(|(_, record)| {
          record.unique_address != remote_record.unique_address
            && record.authority == remote_record.authority
            && record.status.is_active()
            && record.join_version < remote_record.join_version
        })
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
      for key in superseded_keys {
        if let Some(record) = records.get_mut(&key) {
          record.status = NodeStatus::Dead;
          record.version = max_version(record.version, remote_record.version);
          outcome.stale_records_suppressed.push(record.clone());
        }
      }

      match records.get(&remote_record.unique_address).cloned() {
        | Some(local_record) => {
          let preferred = preferred_record(local_record.clone(), remote_record.clone());
          if local_record.version == remote_record.version && local_record != remote_record {
            let ignored = if preferred == remote_record { local_record.clone() } else { remote_record.clone() };
            outcome.conflicts.push(GossipMergeConflict::new(preferred.clone(), ignored));
          }
          if preferred == remote_record && preferred != local_record {
            outcome.applied_records.push(remote_record.clone());
            records.insert(remote_record.unique_address.clone(), remote_record);
          }
        },
        | None => {
          outcome.applied_records.push(remote_record.clone());
          records.insert(remote_record.unique_address.clone(), remote_record);
        },
      }
    }

    let entries = records.values().cloned().collect::<Vec<_>>();
    let terminal_tombstones = GossipTombstoneSet::from_records(&entries);
    outcome.tombstones_added.extend(self.snapshot.tombstones.merge(&terminal_tombstones));

    self.snapshot.membership = MembershipSnapshot::new_with_reachability(
      max_version(self.snapshot.membership.version, remote.membership.version),
      entries,
      merge_reachability(&self.snapshot.membership.reachability, &remote.membership.reachability),
    );

    outcome
  }

  /// Prunes tombstones whose versions have been retained through convergence.
  pub fn prune_retained_tombstones(&mut self, retained_through: MembershipVersion) -> GossipTombstonePruneOutcome {
    let pruned = self.snapshot.tombstones.prune_retained(retained_through);
    for tombstone in &pruned {
      self.snapshot.membership.entries.retain(|record| {
        record.unique_address != tombstone.member
          || !matches!(record.status, NodeStatus::Removed | NodeStatus::Dead)
          || record.version > tombstone.version
      });
      self
        .snapshot
        .membership
        .reachability
        .records
        .retain(|record| record.observer != tombstone.member && record.subject != tombstone.member);
      self.snapshot.membership.reachability.observer_versions.remove(&tombstone.member);
    }
    GossipTombstonePruneOutcome::new(pruned)
  }

  /// Marks a peer identity as having observed a membership version.
  pub fn mark_seen(&mut self, peer: UniqueAddress, version: MembershipVersion) -> bool {
    self.snapshot.seen_digest.mark_seen(peer, version)
  }

  /// Returns the current seen digest.
  #[must_use]
  pub const fn seen_digest(&self) -> &GossipSeenDigest {
    &self.snapshot.seen_digest
  }

  /// Returns true when all active peers have observed at least `version`.
  #[must_use]
  pub fn has_seen_all(&self, active_peers: &[UniqueAddress], version: MembershipVersion) -> bool {
    self.snapshot.seen_digest.has_seen_all(active_peers, version)
  }

  /// Prunes tombstones after all active peers have observed `version`.
  pub fn prune_tombstones_when_seen_by_all(
    &mut self,
    active_peers: &[UniqueAddress],
    version: MembershipVersion,
  ) -> GossipTombstonePruneOutcome {
    if self.has_seen_all(active_peers, version) {
      self.prune_retained_tombstones(version)
    } else {
      GossipTombstonePruneOutcome::new(Vec::new())
    }
  }
}

fn preferred_record(left: NodeRecord, right: NodeRecord) -> NodeRecord {
  if left.version > right.version {
    return left;
  }
  if right.version > left.version {
    return right;
  }
  if status_rank(left.status) > status_rank(right.status) {
    return left;
  }
  if status_rank(right.status) > status_rank(left.status) {
    return right;
  }
  if record_tie_breaks_left(&left, &right) { left } else { right }
}

const fn status_rank(status: NodeStatus) -> u8 {
  match status {
    | NodeStatus::Dead => 9,
    | NodeStatus::Removed => 8,
    | NodeStatus::Exiting => 7,
    | NodeStatus::Leaving => 6,
    | NodeStatus::ReadyForShutdown => 5,
    | NodeStatus::PreparingForShutdown => 4,
    | NodeStatus::Suspect => 3,
    | NodeStatus::Up => 2,
    | NodeStatus::WeaklyUp => 1,
    | NodeStatus::Joining => 0,
  }
}

const fn is_live_incarnation_status(status: NodeStatus) -> bool {
  !matches!(status, NodeStatus::Removed | NodeStatus::Dead)
}

fn record_tie_breaks_left(left: &NodeRecord, right: &NodeRecord) -> bool {
  if left.join_version != right.join_version {
    return left.join_version > right.join_version;
  }
  if left.node_id != right.node_id {
    return left.node_id > right.node_id;
  }
  if left.authority != right.authority {
    return left.authority > right.authority;
  }
  if left.data_center != right.data_center {
    return left.data_center > right.data_center;
  }
  if left.app_version != right.app_version {
    return left.app_version > right.app_version;
  }
  left.roles >= right.roles
}

fn max_version(left: MembershipVersion, right: MembershipVersion) -> MembershipVersion {
  if left >= right { left } else { right }
}

fn merge_reachability(left: &ReachabilitySnapshot, right: &ReachabilitySnapshot) -> ReachabilitySnapshot {
  let mut records = left
    .records
    .iter()
    .cloned()
    .map(|record| ((record.observer.clone(), record.subject.clone()), record))
    .collect::<BTreeMap<(UniqueAddress, UniqueAddress), ReachabilityRecord>>();

  for (remote_observer, remote_version) in &right.observer_versions {
    records.retain(|(observer, subject), record| {
      observer != remote_observer
        || record.version >= *remote_version
        || right
          .records
          .iter()
          .any(|right_record| &right_record.observer == observer && &right_record.subject == subject)
    });
  }

  for record in right.records.iter().cloned() {
    let key = (record.observer.clone(), record.subject.clone());
    if left.observer_versions.get(&record.observer).is_some_and(|local_version| *local_version > record.version)
      && !records.contains_key(&key)
    {
      continue;
    }
    match records.get(&key) {
      | Some(existing) if existing.version > record.version => {},
      | Some(existing)
        if existing.version == record.version
          && reachability_status_rank(existing.status) >= reachability_status_rank(record.status) => {},
      | _ => {
        records.insert(key, record);
      },
    }
  }

  let mut observer_versions = left.observer_versions.clone();
  for (observer, version) in right.observer_versions.iter() {
    let entry = observer_versions.entry(observer.clone()).or_insert(0);
    if *entry < *version {
      *entry = *version;
    }
  }

  ReachabilitySnapshot::new(records.values().cloned().collect(), observer_versions)
}

const fn reachability_status_rank(status: ReachabilityStatus) -> u8 {
  match status {
    | ReachabilityStatus::Terminated => 2,
    | ReachabilityStatus::Unreachable => 1,
    | ReachabilityStatus::Reachable => 0,
  }
}
