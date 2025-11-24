//! Snapshot of cluster topology changes used for event publication.

use alloc::{string::String, vec::Vec};

/// Topology delta communicated to the cluster core.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClusterTopology {
  hash:   u64,
  joined: Vec<String>,
  left:   Vec<String>,
}

impl ClusterTopology {
  /// Creates a new topology snapshot.
  #[must_use]
  pub const fn new(hash: u64, joined: Vec<String>, left: Vec<String>) -> Self {
    Self { hash, joined, left }
  }

  /// Topology hash value.
  #[must_use]
  pub const fn hash(&self) -> u64 {
    self.hash
  }

  /// Joined member addresses.
  #[must_use]
  pub const fn joined(&self) -> &Vec<String> {
    &self.joined
  }

  /// Left member addresses.
  #[must_use]
  pub const fn left(&self) -> &Vec<String> {
    &self.left
  }
}
