//! Journal plugin trait.

use alloc::vec::Vec;

use crate::core::{journal_error::JournalError, persistent_repr::PersistentRepr};

/// Journal interface used by persistent actors.
pub trait Journal: Send + Sync + 'static {
  /// Writes the provided messages to the journal.
  ///
  /// # Errors
  ///
  /// Returns an error when the journal fails to persist the messages.
  fn write_messages(&mut self, messages: &[PersistentRepr]) -> Result<(), JournalError>;

  /// Replays messages for the given persistence id.
  ///
  /// Returns the replayed messages together with the highest stored sequence number.
  ///
  /// # Errors
  ///
  /// Returns an error when the replay cannot be completed.
  fn replay_messages(
    &self,
    persistence_id: &str,
    from_sequence_nr: u64,
    to_sequence_nr: u64,
    max: u64,
  ) -> Result<(Vec<PersistentRepr>, u64), JournalError>;

  /// Deletes messages up to the provided sequence number (inclusive).
  ///
  /// # Errors
  ///
  /// Returns an error when deletion fails.
  fn delete_messages_to(&mut self, persistence_id: &str, to_sequence_nr: u64) -> Result<(), JournalError>;

  /// Returns the highest stored sequence number.
  ///
  /// # Errors
  ///
  /// Returns an error when the query fails.
  fn highest_sequence_nr(&self, persistence_id: &str) -> Result<u64, JournalError>;
}
