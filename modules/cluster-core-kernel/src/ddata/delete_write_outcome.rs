//! Write outcome vocabulary for distributed-data delete commands.

/// Result of applying the write policy after a local delete has been accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteWriteOutcome {
  /// The local delete and requested write policy completed.
  Success,
  /// The local delete was accepted, but requested replication did not complete in time.
  ReplicationFailure,
  /// The local delete was accepted, but durable storage failed.
  StoreFailure,
}
