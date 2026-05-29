//! Failure observation kind.

/// Availability observation state before a downing decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureObservationKind {
  /// The authority is suspected but not departed.
  Suspect,
  /// The authority is currently unreachable but not departed.
  Unreachable,
  /// The authority became reachable again before a downing decision.
  Recovered,
}
