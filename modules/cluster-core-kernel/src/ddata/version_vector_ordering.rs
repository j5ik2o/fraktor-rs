//! Causal ordering outcome for version vectors.

/// Causal ordering between two version vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionVectorOrdering {
  /// Both vectors contain the same history.
  Same,
  /// The left vector happened before the right vector.
  Before,
  /// The left vector happened after the right vector.
  After,
  /// Both vectors contain independent history.
  Concurrent,
}
