//! Removed-node pruning contract for CRDT values.

use alloc::collections::BTreeSet;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::ReplicatedData;

/// CRDT value that can collapse and remove contributions from departed nodes.
pub trait RemovedNodePruning: ReplicatedData {
  /// Error returned when node contribution cannot be collapsed safely.
  type PruneError;

  /// Returns all nodes that have contributed to this value.
  #[must_use]
  fn modified_by_nodes(&self) -> BTreeSet<UniqueAddress>;

  /// Returns true when this value contains contribution from `removed_node`.
  #[must_use]
  fn need_pruning_from(&self, removed_node: &UniqueAddress) -> bool;

  /// Moves contribution from `removed_node` into `collapse_into`.
  ///
  /// # Errors
  ///
  /// Returns [`Self::PruneError`] when the collapsed value cannot be represented.
  fn prune(&self, removed_node: &UniqueAddress, collapse_into: &UniqueAddress) -> Result<Self, Self::PruneError>;

  /// Removes residual contribution from `removed_node`.
  #[must_use]
  fn pruning_cleanup(&self, removed_node: &UniqueAddress) -> Self;
}
