//! Snapshot payload for reachability matrix state.

use alloc::{collections::BTreeMap, vec::Vec};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{ReachabilityRecord, ReachabilityStatus};

/// Immutable snapshot of reachability records and observer row versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachabilitySnapshot {
  /// Non-default reachability records.
  pub records:           Vec<ReachabilityRecord>,
  /// Latest version observed for each observer row.
  pub observer_versions: BTreeMap<UniqueAddress, u64>,
}

impl ReachabilitySnapshot {
  /// Creates a reachability snapshot.
  #[must_use]
  pub const fn new(records: Vec<ReachabilityRecord>, observer_versions: BTreeMap<UniqueAddress, u64>) -> Self {
    Self { records, observer_versions }
  }

  /// Creates an empty reachability snapshot.
  #[must_use]
  pub const fn empty() -> Self {
    Self { records: Vec::new(), observer_versions: BTreeMap::new() }
  }

  /// Returns the aggregate status for a subject across all observers.
  #[must_use]
  pub fn aggregate_status(&self, subject: &UniqueAddress) -> ReachabilityStatus {
    let mut aggregate = ReachabilityStatus::Reachable;
    for record in self.records.iter().filter(|record| &record.subject == subject) {
      match record.status {
        | ReachabilityStatus::Terminated => return ReachabilityStatus::Terminated,
        | ReachabilityStatus::Unreachable => aggregate = ReachabilityStatus::Unreachable,
        | ReachabilityStatus::Reachable => {},
      }
    }
    aggregate
  }
}
