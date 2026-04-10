//! Snapshot of placement coordinator state.

use alloc::{string::String, vec::Vec};

use super::placement_coordinator_state::PlacementCoordinatorState;

/// Snapshot of placement coordinator metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementSnapshot {
  /// Current coordinator state.
  pub state:           PlacementCoordinatorState,
  /// Known authority list.
  pub authorities:     Vec<String>,
  /// Local authority identifier if configured.
  pub local_authority: Option<String>,
}
