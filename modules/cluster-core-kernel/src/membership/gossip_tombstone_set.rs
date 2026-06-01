//! Tombstone collection keyed by member identity.

use alloc::{collections::BTreeMap, vec::Vec};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{GossipTombstone, MembershipVersion, NodeRecord, NodeStatus};

/// Deterministic tombstone set for removed or dead members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipTombstoneSet {
  entries: BTreeMap<UniqueAddress, GossipTombstone>,
}

impl GossipTombstoneSet {
  /// Creates an empty tombstone set.
  #[must_use]
  pub const fn new() -> Self {
    Self { entries: BTreeMap::new() }
  }

  /// Creates tombstones from terminal membership records.
  #[must_use]
  pub fn from_records(records: &[NodeRecord]) -> Self {
    let mut set = Self::new();
    for record in records {
      set.insert_terminal_record(record);
    }
    set
  }

  /// Returns the tombstone for a member identity.
  #[must_use]
  pub fn get(&self, member: &UniqueAddress) -> Option<&GossipTombstone> {
    self.entries.get(member)
  }

  /// Returns tombstones in deterministic identity order.
  #[must_use]
  pub fn values(&self) -> Vec<GossipTombstone> {
    self.entries.values().cloned().collect()
  }

  /// Inserts a tombstone, keeping the newest version for the member.
  pub fn insert(&mut self, tombstone: GossipTombstone) -> bool {
    match self.entries.get(&tombstone.member) {
      | Some(existing) if existing.version >= tombstone.version => false,
      | _ => {
        self.entries.insert(tombstone.member.clone(), tombstone);
        true
      },
    }
  }

  /// Inserts a tombstone when the record is terminal.
  pub fn insert_terminal_record(&mut self, record: &NodeRecord) -> bool {
    if is_terminal(record.status) {
      return self.insert(GossipTombstone::new(record.unique_address.clone(), record.version));
    }
    false
  }

  /// Merges another tombstone set and returns newly applied tombstones.
  pub fn merge(&mut self, other: &Self) -> Vec<GossipTombstone> {
    let mut applied = Vec::new();
    for tombstone in other.entries.values() {
      if self.insert(tombstone.clone()) {
        applied.push(tombstone.clone());
      }
    }
    applied
  }

  /// Returns true when the tombstone suppresses this stale record.
  #[must_use]
  pub fn suppresses(&self, record: &NodeRecord) -> bool {
    self.entries.get(&record.unique_address).is_some_and(|tombstone| {
      tombstone.version > record.version || tombstone.version == record.version && !is_terminal(record.status)
    })
  }

  /// Prunes tombstones whose versions are retained through convergence.
  pub fn prune_retained(&mut self, retained_through: MembershipVersion) -> Vec<GossipTombstone> {
    let keys = self
      .entries
      .iter()
      .filter(|(_, tombstone)| tombstone.version <= retained_through)
      .map(|(member, _)| member.clone())
      .collect::<Vec<_>>();
    let mut pruned = Vec::new();
    for key in keys {
      if let Some(tombstone) = self.entries.remove(&key) {
        pruned.push(tombstone);
      }
    }
    pruned
  }
}

impl Default for GossipTombstoneSet {
  fn default() -> Self {
    Self::new()
  }
}

const fn is_terminal(status: NodeStatus) -> bool {
  matches!(status, NodeStatus::Removed | NodeStatus::Dead)
}
