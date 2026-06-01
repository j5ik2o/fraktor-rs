//! Snapshot payload for reachability matrix state.

use alloc::{collections::BTreeMap, vec::Vec};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::ReachabilityRecord;

/// Immutable snapshot of reachability records and observer row versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachabilitySnapshot {
  /// Non-default reachability records.
  pub records:           Vec<ReachabilityRecord>,
  /// Latest version observed for each observer row.
  pub observer_versions: BTreeMap<UniqueAddress, u64>,
}
