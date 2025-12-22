//! Placement resolution result.

use alloc::string::String;

use crate::core::{placement_decision::PlacementDecision, placement_locality::PlacementLocality};

/// Finalized placement resolution for a grain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementResolution {
  /// Decision metadata.
  pub decision: PlacementDecision,
  /// Local or remote locality.
  pub locality: PlacementLocality,
  /// Resolved PID string.
  pub pid:      String,
}
