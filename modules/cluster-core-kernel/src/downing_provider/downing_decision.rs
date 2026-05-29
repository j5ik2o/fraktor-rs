//! Decision returned by a downing strategy.

/// Decision produced by the core downing boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DowningDecision {
  /// Remove the authority from active topology.
  Down,
  /// Keep the authority in active topology.
  Keep,
  /// Leave the authority unchanged until more evidence is available.
  Defer,
}
