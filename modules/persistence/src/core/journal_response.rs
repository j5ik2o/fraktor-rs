//! Responses emitted by journal actors.

#[cfg(test)]
mod tests;

use alloc::string::String;

use crate::core::{journal_error::JournalError, persistent_repr::PersistentRepr};

/// Responses from journal operations.
#[derive(Clone, Debug)]
pub enum JournalResponse {
  /// Single message write succeeded.
  WriteMessageSuccess {
    /// Persisted representation.
    repr:        PersistentRepr,
    /// Instance id for correlation.
    instance_id: u32,
  },
  /// Single message write failed.
  WriteMessageFailure {
    /// Failed representation.
    repr:        PersistentRepr,
    /// Failure cause.
    cause:       JournalError,
    /// Instance id for correlation.
    instance_id: u32,
  },
  /// Single message write rejected.
  WriteMessageRejected {
    /// Rejected representation.
    repr:        PersistentRepr,
    /// Rejection reason.
    cause:       JournalError,
    /// Instance id for correlation.
    instance_id: u32,
  },
  /// Batch write succeeded.
  WriteMessagesSuccessful,
  /// Batch write failed.
  WriteMessagesFailed {
    /// Failure cause.
    cause:       JournalError,
    /// Number of writes attempted.
    write_count: u64,
  },
  /// Replayed message entry.
  ReplayedMessage {
    /// Replayed representation.
    persistent_repr: PersistentRepr,
  },
  /// Recovery completed with highest sequence number.
  RecoverySuccess {
    /// Highest sequence number after recovery.
    highest_sequence_nr: u64,
  },
  /// Highest sequence number response.
  HighestSequenceNr {
    /// Persistence id for the response.
    persistence_id: String,
    /// Highest sequence number.
    sequence_nr:    u64,
  },
  /// Highest sequence number lookup failed.
  HighestSequenceNrFailure {
    /// Persistence id for the response.
    persistence_id: String,
    /// Failure cause.
    cause:          JournalError,
  },
  /// Replay failed.
  ReplayMessagesFailure {
    /// Failure cause.
    cause: JournalError,
  },
  /// Delete messages succeeded.
  DeleteMessagesSuccess {
    /// Upper bound of deletion.
    to_sequence_nr: u64,
  },
  /// Delete messages failed.
  DeleteMessagesFailure {
    /// Failure cause.
    cause:          JournalError,
    /// Upper bound of deletion.
    to_sequence_nr: u64,
  },
}
