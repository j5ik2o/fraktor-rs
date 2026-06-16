//! Write outcome vocabulary for distributed-data update commands.

/// Result of applying the write policy after a local update has been accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateWriteOutcome {
  /// The local update and requested write policy completed.
  Success,
  /// The local update was accepted, but requested replication did not complete in time.
  Timeout,
  /// The local update was accepted, but durable storage failed.
  StoreFailure,
}
