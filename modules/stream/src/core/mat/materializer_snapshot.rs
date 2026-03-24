#[cfg(test)]
mod tests;

use super::MaterializerLifecycleState;

/// Diagnostic snapshot of a materializer's current state.
///
/// Provides a point-in-time view of the materializer for monitoring
/// and debugging purposes. Equivalent to a subset of Pekko's
/// `MaterializerState.streamSnapshots()`.
#[derive(Debug, Clone)]
pub struct MaterializerSnapshot {
  lifecycle_state:    MaterializerLifecycleState,
  total_materialized: u64,
}

impl MaterializerSnapshot {
  /// Creates a new snapshot.
  pub(crate) const fn new(lifecycle_state: MaterializerLifecycleState, total_materialized: u64) -> Self {
    Self { lifecycle_state, total_materialized }
  }

  /// Returns the current lifecycle state of the materializer.
  #[must_use]
  pub const fn lifecycle_state(&self) -> MaterializerLifecycleState {
    self.lifecycle_state
  }

  /// Returns the total number of graphs materialized since creation.
  #[must_use]
  pub const fn total_materialized(&self) -> u64 {
    self.total_materialized
  }
}
