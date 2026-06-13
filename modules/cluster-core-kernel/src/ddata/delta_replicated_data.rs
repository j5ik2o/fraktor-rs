//! Delta CRDT contract.

use super::{ReplicatedData, ReplicatedDelta};

/// CRDT value that can expose and merge accumulated deltas.
pub trait DeltaReplicatedData: ReplicatedData {
  /// Delta payload type emitted by this full state.
  type Delta: ReplicatedDelta<Full = Self>;

  /// Returns the accumulated delta since the last reset.
  #[must_use]
  fn delta(&self) -> Option<Self::Delta>;

  /// Merges a delta into this full state.
  #[must_use]
  fn merge_delta(&self, delta: &Self::Delta) -> Self;

  /// Returns the same full state with the accumulated delta cleared.
  #[must_use]
  fn reset_delta(&self) -> Self;
}
