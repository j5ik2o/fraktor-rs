//! Atomic journal write boundary.

#[cfg(test)]
#[path = "atomic_write_test.rs"]
mod tests;

use alloc::{string::ToString, vec::Vec};

use crate::persistent::{AtomicWriteError, PersistentRepr};

/// All-or-none journal write unit for one persistence id.
#[derive(Clone, Debug)]
pub struct AtomicWrite {
  payload: Vec<PersistentRepr>,
}

impl AtomicWrite {
  /// Creates an atomic write after validating the journal atomicity invariant.
  ///
  /// # Errors
  ///
  /// Returns [`AtomicWriteError::Empty`] for empty payloads and
  /// [`AtomicWriteError::MixedPersistenceId`] when entries contain different persistence ids.
  pub fn new(payload: Vec<PersistentRepr>) -> Result<Self, AtomicWriteError> {
    let Some(first) = payload.first() else {
      return Err(AtomicWriteError::Empty);
    };
    let persistence_id = first.persistence_id();
    for repr in &payload {
      if repr.persistence_id() != persistence_id {
        return Err(AtomicWriteError::MixedPersistenceId {
          expected: persistence_id.to_string(),
          actual:   repr.persistence_id().to_string(),
        });
      }
    }
    Ok(Self { payload })
  }

  /// Returns the persistence id shared by all payload entries.
  #[must_use]
  pub fn persistence_id(&self) -> &str {
    self.payload[0].persistence_id()
  }

  /// Returns the lowest sequence number in the payload.
  #[must_use]
  pub fn lowest_sequence_nr(&self) -> u64 {
    self.payload.iter().map(PersistentRepr::sequence_nr).min().unwrap_or(0)
  }

  /// Returns the highest sequence number in the payload.
  #[must_use]
  pub fn highest_sequence_nr(&self) -> u64 {
    self.payload.iter().map(PersistentRepr::sequence_nr).max().unwrap_or(0)
  }

  /// Returns the number of persistent representations in the atomic write.
  #[must_use]
  pub const fn size(&self) -> usize {
    self.payload.len()
  }

  /// Returns `true` when the atomic write contains no payload entries.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.payload.is_empty()
  }

  /// Returns the contained persistent representations.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub fn payload(&self) -> &[PersistentRepr] {
    &self.payload
  }

  /// Consumes the atomic write and returns its payload.
  #[must_use]
  pub fn into_payload(self) -> Vec<PersistentRepr> {
    self.payload
  }
}
