//! Delta payload contract for CRDT values.

use super::{DeltaReplicatedData, ReplicatedData};

/// Delta payload that can provide an empty full state for first delivery.
pub trait ReplicatedDelta: ReplicatedData {
  /// Full state type represented by this delta.
  type Full: DeltaReplicatedData<Delta = Self>;

  /// Returns an empty full state that can receive this delta.
  #[must_use]
  fn zero(&self) -> Self::Full;
}
