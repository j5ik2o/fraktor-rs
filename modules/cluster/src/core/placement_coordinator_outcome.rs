//! Placement coordinator outcome.

use alloc::vec::Vec;

use crate::core::{
  placement_command::PlacementCommand, placement_event::PlacementEvent, placement_resolution::PlacementResolution,
};

/// Result of placement coordination.
#[derive(Debug, Default)]
pub struct PlacementCoordinatorOutcome {
  /// Finalized resolution, if available.
  pub resolution: Option<PlacementResolution>,
  /// Commands to execute via driver.
  pub commands:   Vec<PlacementCommand>,
  /// Events generated during processing.
  pub events:     Vec<PlacementEvent>,
}
