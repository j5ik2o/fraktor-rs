//! Placement event types for observability.

use alloc::string::String;

use crate::core::grain::GrainKey;

/// Events emitted during placement resolution and activation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementEvent {
  /// Placement resolution completed.
  Resolved {
    /// Target grain key.
    key:         GrainKey,
    /// Selected authority.
    authority:   String,
    /// Observation timestamp in seconds.
    observed_at: u64,
  },
  /// Lock acquisition denied.
  LockDenied {
    /// Target grain key.
    key:         GrainKey,
    /// Denial reason.
    reason:      String,
    /// Observation timestamp in seconds.
    observed_at: u64,
  },
  /// Activation created or reused.
  Activated {
    /// Target grain key.
    key:         GrainKey,
    /// PID string.
    pid:         String,
    /// Observation timestamp in seconds.
    observed_at: u64,
  },
  /// Activation was passivated.
  Passivated {
    /// Target grain key.
    key:         GrainKey,
    /// Observation timestamp in seconds.
    observed_at: u64,
  },
}
