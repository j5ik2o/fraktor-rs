//! Errors raised when applying topology updates.

use alloc::string::String;

/// Topology apply failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyApplyError {
  /// Cluster is not started.
  NotStarted,
  /// Invalid topology update detected.
  InvalidTopology {
    /// Failure reason.
    reason: String,
  },
}
