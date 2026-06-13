//! Replica-count response protocol vocabulary.

#[cfg(test)]
#[path = "replica_count_test.rs"]
mod tests;

/// Response carrying the replica count including the local node.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplicaCount {
  n: usize,
}

impl ReplicaCount {
  /// Creates a replica-count response.
  #[must_use]
  pub const fn new(n: usize) -> Self {
    Self { n }
  }

  /// Returns the replica count.
  #[must_use]
  pub const fn get(&self) -> usize {
    self.n
  }
}
