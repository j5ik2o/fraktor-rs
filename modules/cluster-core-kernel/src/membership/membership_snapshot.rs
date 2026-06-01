//! Membership snapshot for handshake.

#[cfg(test)]
#[path = "membership_snapshot_test.rs"]
mod tests;

use alloc::vec::Vec;

use super::{DataCenter, MembershipVersion, NodeRecord, ReachabilitySnapshot};

/// Immutable view of the membership table used during handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipSnapshot {
  /// Version when the snapshot was taken.
  pub version:      MembershipVersion,
  /// Complete list of node records.
  pub entries:      Vec<NodeRecord>,
  /// Reachability evidence known when the snapshot was taken.
  pub reachability: ReachabilitySnapshot,
}

impl MembershipSnapshot {
  /// Creates a new snapshot.
  #[must_use]
  pub const fn new(version: MembershipVersion, entries: Vec<NodeRecord>) -> Self {
    Self { version, entries, reachability: ReachabilitySnapshot::empty() }
  }

  /// Creates a new snapshot with reachability evidence.
  #[must_use]
  pub const fn new_with_reachability(
    version: MembershipVersion,
    entries: Vec<NodeRecord>,
    reachability: ReachabilitySnapshot,
  ) -> Self {
    Self { version, entries, reachability }
  }

  /// Returns members that belong to the given data center.
  #[must_use]
  pub fn members_in_data_center(&self, data_center: &DataCenter) -> Vec<NodeRecord> {
    self.entries.iter().filter(|record| &record.data_center == data_center).cloned().collect()
  }
}
