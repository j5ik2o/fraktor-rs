//! Journal storage abstraction.

use core::future::Future;

use crate::core::{journal_error::JournalError, persistent_repr::PersistentRepr};

/// Event journal abstraction using GATs for no_std async.
pub trait Journal: Send + Sync + 'static {
  /// Future returned by write operations.
  type WriteFuture<'a>: Future<Output = Result<(), JournalError>> + Send + 'a
  where
    Self: 'a;

  /// Future returned by replay operations.
  type ReplayFuture<'a>: Future<Output = Result<alloc::vec::Vec<PersistentRepr>, JournalError>> + Send + 'a
  where
    Self: 'a;

  /// Future returned by delete operations.
  type DeleteFuture<'a>: Future<Output = Result<(), JournalError>> + Send + 'a
  where
    Self: 'a;

  /// Future returned by highest sequence number lookup.
  type HighestSeqNrFuture<'a>: Future<Output = Result<u64, JournalError>> + Send + 'a
  where
    Self: 'a;

  /// Writes a batch of messages.
  fn write_messages<'a>(&'a mut self, messages: &'a [PersistentRepr]) -> Self::WriteFuture<'a>;

  /// Replays messages in the requested range.
  fn replay_messages<'a>(
    &'a self,
    persistence_id: &'a str,
    from_sequence_nr: u64,
    to_sequence_nr: u64,
    max: u64,
  ) -> Self::ReplayFuture<'a>;

  /// Deletes messages up to the given sequence number.
  fn delete_messages_to<'a>(&'a mut self, persistence_id: &'a str, to_sequence_nr: u64) -> Self::DeleteFuture<'a>;

  /// Returns the highest sequence number for the persistence id.
  fn highest_sequence_nr<'a>(&'a self, persistence_id: &'a str) -> Self::HighestSeqNrFuture<'a>;
}
