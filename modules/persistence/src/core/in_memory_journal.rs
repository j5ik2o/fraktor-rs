//! In-memory journal implementation.

use alloc::{string::String, vec::Vec};

use ahash::RandomState;
use hashbrown::HashMap;

use crate::core::{journal::Journal, journal_error::JournalError, persistent_repr::PersistentRepr};

/// In-memory journal storing events per persistence id.
pub struct InMemoryJournal {
  entries: HashMap<String, Vec<PersistentRepr>, RandomState>,
}

impl InMemoryJournal {
  /// Creates a new empty journal.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: HashMap::with_hasher(RandomState::new()) }
  }
}

impl Default for InMemoryJournal {
  fn default() -> Self {
    Self::new()
  }
}

impl Journal for InMemoryJournal {
  fn write_messages(&mut self, messages: &[PersistentRepr]) -> Result<(), JournalError> {
    for message in messages {
      let persistence_id = String::from(message.persistence_id());
      let entry = self.entries.entry(persistence_id).or_insert_with(Vec::new);
      let expected = entry.last().map(|last| last.sequence_nr().saturating_add(1)).unwrap_or(1);
      if message.sequence_nr() != expected {
        return Err(JournalError::SequenceMismatch { expected, actual: message.sequence_nr() });
      }
      entry.push(message.clone());
    }
    Ok(())
  }

  fn replay_messages(
    &self,
    persistence_id: &str,
    from_sequence_nr: u64,
    to_sequence_nr: u64,
    max: u64,
  ) -> Result<(Vec<PersistentRepr>, u64), JournalError> {
    let mut replayed = Vec::new();
    let mut highest = 0;
    if let Some(entry) = self.entries.get(persistence_id) {
      highest = entry.last().map(|last| last.sequence_nr()).unwrap_or(0);
      let max = usize::try_from(max).unwrap_or(usize::MAX);
      for message in entry.iter().filter(|message| {
        let seq = message.sequence_nr();
        seq >= from_sequence_nr && seq <= to_sequence_nr
      }) {
        if replayed.len() >= max {
          break;
        }
        replayed.push(message.clone());
      }
    }
    Ok((replayed, highest))
  }

  fn delete_messages_to(&mut self, persistence_id: &str, to_sequence_nr: u64) -> Result<(), JournalError> {
    if let Some(entry) = self.entries.get_mut(persistence_id) {
      entry.retain(|message| message.sequence_nr() > to_sequence_nr);
    }
    Ok(())
  }

  fn highest_sequence_nr(&self, persistence_id: &str) -> Result<u64, JournalError> {
    Ok(self.entries.get(persistence_id).and_then(|entry| entry.last().map(|last| last.sequence_nr())).unwrap_or(0))
  }
}
