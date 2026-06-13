//! Write consistency vocabulary for future Replicator operations.

#[cfg(test)]
#[path = "write_consistency_test.rs"]
mod tests;

use core::time::Duration;

/// Write-side consistency level requested by a distributed-data write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteConsistency {
  /// Write to the local replica only.
  Local,
  /// Write to at least `n` replicas before `timeout`.
  To {
    /// Required number of replicas.
    n:       usize,
    /// Maximum time to wait.
    timeout: Duration,
  },
  /// Write to a majority with an optional minimum quorum cap.
  Majority {
    /// Maximum time to wait.
    timeout: Duration,
    /// Minimum quorum cap.
    min_cap: usize,
  },
  /// Write to a majority plus additional replicas.
  MajorityPlus {
    /// Maximum time to wait.
    timeout:    Duration,
    /// Additional replicas beyond majority.
    additional: usize,
    /// Minimum quorum cap.
    min_cap:    usize,
  },
  /// Write to all known replicas before `timeout`.
  All {
    /// Maximum time to wait.
    timeout: Duration,
  },
}
