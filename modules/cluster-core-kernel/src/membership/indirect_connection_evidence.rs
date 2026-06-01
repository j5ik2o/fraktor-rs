//! Evidence for indirect or partial connectivity.

#[cfg(test)]
#[path = "indirect_connection_evidence_test.rs"]
mod tests;

use alloc::vec::Vec;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::ReachabilityRecord;

/// Partial connectivity evidence generated from reachability observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndirectConnectionEvidence {
  /// Subject whose connectivity is being evaluated.
  pub subject:                     UniqueAddress,
  /// Direct non-reachable observations for the subject.
  pub direct_observations:         Vec<ReachabilityRecord>,
  /// Indirect reachable observations from other observers.
  pub indirect_observations:       Vec<ReachabilityRecord>,
  /// Aggregate reachability status for observers participating in the evidence.
  pub observer_aggregate_statuses: Vec<ReachabilityRecord>,
}
