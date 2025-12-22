//! Placement lock lease representation.

use alloc::string::String;

use crate::core::grain_key::GrainKey;

/// Represents an acquired placement lock lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementLease {
  /// Locked grain key.
  pub key:        GrainKey,
  /// Lease owner (authority).
  pub owner:      String,
  /// Lease expiration timestamp in seconds.
  pub expires_at: u64,
}
