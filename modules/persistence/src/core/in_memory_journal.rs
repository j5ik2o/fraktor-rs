//! In-memory journal implementation for testing.

#[cfg(test)]
mod tests;

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};
use core::future::{Ready, ready};

use crate::core::{journal::Journal, journal_error::JournalError, persistent_repr::PersistentRepr};

/// In-memory journal implementation.
#[derive(Clone, Debug, Default)]
pub struct InMemoryJournal {
  entries:              BTreeMap<String, Vec<PersistentRepr>>,
  highest_sequence_nrs: BTreeMap<String, u64>,
}

impl InMemoryJournal {
  /// Creates a new in-memory journal.
  #[must_use]
  pub const fn new() -> Self {
    Self { entries: BTreeMap::new(), highest_sequence_nrs: BTreeMap::new() }
  }

  fn expected_sequence_nr(&self, persistence_id: &str) -> u64 {
    self.highest_sequence_nrs.get(persistence_id).copied().unwrap_or(0).saturating_add(1)
  }
}

impl Journal for InMemoryJournal {
  type DeleteFuture<'a>
    = Ready<Result<(), JournalError>>
  where
    Self: 'a;
  type HighestSeqNrFuture<'a>
    = Ready<Result<u64, JournalError>>
  where
    Self: 'a;
  type ReplayFuture<'a>
    = Ready<Result<Vec<PersistentRepr>, JournalError>>
  where
    Self: 'a;
  type WriteFuture<'a>
    = Ready<Result<(), JournalError>>
  where
    Self: 'a;

  fn write_messages<'a>(&'a mut self, messages: &'a [PersistentRepr]) -> Self::WriteFuture<'a> {
    let Some(first) = messages.first() else {
      return ready(Ok(()));
    };

    let persistence_id = first.persistence_id().to_string();
    let mut expected = self.expected_sequence_nr(&persistence_id);

    for message in messages {
      if message.sequence_nr() != expected {
        return ready(Err(JournalError::SequenceMismatch { expected, actual: message.sequence_nr() }));
      }
      expected = expected.saturating_add(1);
    }

    let entry = self.entries.entry(persistence_id.clone()).or_default();
    entry.extend(messages.iter().cloned());
    self.highest_sequence_nrs.insert(persistence_id, expected.saturating_sub(1));

    ready(Ok(()))
  }

  fn replay_messages<'a>(
    &'a self,
    persistence_id: &'a str,
    from_sequence_nr: u64,
    to_sequence_nr: u64,
    max: u64,
  ) -> Self::ReplayFuture<'a> {
    let mut result = Vec::new();
    if let Some(entries) = self.entries.get(persistence_id) {
      for repr in entries.iter().filter(|repr| {
        let sequence_nr = repr.sequence_nr();
        sequence_nr >= from_sequence_nr && sequence_nr <= to_sequence_nr
      }) {
        result.push(repr.clone());
        if max != 0 && result.len() as u64 >= max {
          break;
        }
      }
    }
    ready(Ok(result))
  }

  fn delete_messages_to<'a>(&'a mut self, persistence_id: &'a str, to_sequence_nr: u64) -> Self::DeleteFuture<'a> {
    if let Some(entries) = self.entries.get_mut(persistence_id) {
      entries.retain(|repr| repr.sequence_nr() > to_sequence_nr);
      if entries.is_empty() {
        self.entries.remove(persistence_id);
      }
    }
    ready(Ok(()))
  }

  fn highest_sequence_nr<'a>(&'a self, persistence_id: &'a str) -> Self::HighestSeqNrFuture<'a> {
    ready(Ok(self.highest_sequence_nrs.get(persistence_id).copied().unwrap_or(0)))
  }
}
