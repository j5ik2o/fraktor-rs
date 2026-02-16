//! Placement command definitions for driver execution.

use alloc::string::String;

use super::{ActivationEntry, PlacementLease, PlacementRequestId};
use crate::core::grain::GrainKey;

/// Commands emitted by the coordinator for driver execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementCommand {
  /// Acquire an exclusive lock for the grain key.
  TryAcquire {
    /// Correlation identifier.
    request_id: PlacementRequestId,
    /// Target grain key.
    key:        GrainKey,
    /// Owner authority.
    owner:      String,
    /// Observation timestamp in seconds.
    now:        u64,
  },
  /// Load an activation entry from storage.
  LoadActivation {
    /// Correlation identifier.
    request_id: PlacementRequestId,
    /// Target grain key.
    key:        GrainKey,
  },
  /// Ensure activation for the grain key.
  EnsureActivation {
    /// Correlation identifier.
    request_id: PlacementRequestId,
    /// Target grain key.
    key:        GrainKey,
    /// Owner authority.
    owner:      String,
  },
  /// Store activation entry in storage.
  StoreActivation {
    /// Correlation identifier.
    request_id: PlacementRequestId,
    /// Target grain key.
    key:        GrainKey,
    /// Activation entry.
    entry:      ActivationEntry,
  },
  /// Release the acquired lock.
  Release {
    /// Correlation identifier.
    request_id: PlacementRequestId,
    /// Lease to release.
    lease:      PlacementLease,
  },
}
