//! Placement coordinator error types.

use crate::core::{placement_coordinator_state::PlacementCoordinatorState, placement_request_id::PlacementRequestId};

/// Errors returned by placement coordinator operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementCoordinatorError {
  /// Coordinator has not started yet.
  NotStarted,
  /// Coordinator is in a state that rejects the operation.
  InvalidState {
    /// Current coordinator state.
    state: PlacementCoordinatorState,
  },
  /// Command result did not match a known request.
  UnknownRequest {
    /// Correlation identifier.
    request_id: PlacementRequestId,
  },
}
