//! Factory contract for [`ActorCellStateShared`](super::ActorCellStateShared).

use super::{ActorCellState, ActorCellStateShared};

/// Materializes [`ActorCellStateShared`] instances.
pub trait ActorCellStateSharedFactory: Send + Sync {
  /// Creates a shared actor-cell runtime state wrapper.
  fn create_actor_cell_state_shared(&self, state: ActorCellState) -> ActorCellStateShared;
}
