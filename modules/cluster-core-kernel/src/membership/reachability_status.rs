//! Reachability status observed for a subject member.

/// Reachability state for one observer-subject relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReachabilityStatus {
  /// The subject is reachable, represented by the absence of a matrix record by default.
  Reachable,
  /// The subject is currently unreachable from the observer.
  Unreachable,
  /// The subject is terminated, stronger than an unreachable observation.
  Terminated,
}
