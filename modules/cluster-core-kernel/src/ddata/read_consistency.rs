//! Read consistency vocabulary for future Replicator operations.

#[cfg(test)]
#[path = "read_consistency_test.rs"]
mod tests;

use core::{num::NonZeroUsize, time::Duration};

/// Read-side consistency level requested by a distributed-data read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadConsistency {
  /// Read from the local replica only.
  Local,
  /// Read from at least `n` replicas before `timeout`.
  From {
    /// Required number of replicas.
    n:       NonZeroUsize,
    /// Maximum time to wait.
    timeout: Duration,
  },
  /// Read from a majority with an optional minimum quorum cap.
  Majority {
    /// Maximum time to wait.
    timeout: Duration,
    /// Minimum quorum cap.
    min_cap: usize,
  },
  /// Read from a majority plus additional replicas.
  MajorityPlus {
    /// Maximum time to wait.
    timeout:    Duration,
    /// Additional replicas beyond majority.
    additional: NonZeroUsize,
    /// Minimum quorum cap.
    min_cap:    usize,
  },
  /// Read from all known replicas before `timeout`.
  All {
    /// Maximum time to wait.
    timeout: Duration,
  },
}
