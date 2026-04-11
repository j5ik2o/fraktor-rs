//! Factory contract for [`ActorCellStateShared`](super::ActorCellStateShared).

use super::ActorCellStateShared;

/// Materializes [`ActorCellStateShared`] instances.
pub trait ActorCellStateSharedFactory: Send + Sync {
  /// Creates a shared actor-cell runtime state wrapper.
  fn create(&self) -> ActorCellStateShared;
}
