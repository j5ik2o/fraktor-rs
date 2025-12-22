//! Placement decision details.

use alloc::string::String;

use crate::core::grain_key::GrainKey;

/// Represents a placement decision for a grain key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementDecision {
  /// Target grain key.
  pub key:         GrainKey,
  /// Selected authority.
  pub authority:   String,
  /// Observation timestamp in seconds.
  pub observed_at: u64,
}
