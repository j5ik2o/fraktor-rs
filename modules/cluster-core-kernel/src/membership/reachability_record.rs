//! Observer-subject reachability record.

use fraktor_remote_core_rs::address::UniqueAddress;

use super::ReachabilityStatus;

/// Reachability observation for one observer and one subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachabilityRecord {
  /// Member that produced the observation.
  pub observer: UniqueAddress,
  /// Member whose reachability was observed.
  pub subject:  UniqueAddress,
  /// Observed reachability status.
  pub status:   ReachabilityStatus,
  /// Monotonic version for the observer row at the time of this observation.
  pub version:  u64,
}
