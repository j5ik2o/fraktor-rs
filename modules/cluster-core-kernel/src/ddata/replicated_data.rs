//! Base merge contract for state-based CRDT values.

/// State-based CRDT value that can merge with another value of the same type.
pub trait ReplicatedData: Clone {
  /// Returns the converged value of `self` and `other`.
  #[must_use]
  fn merge(&self, other: &Self) -> Self;
}
