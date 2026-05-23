//! Durable state update record.

#[cfg(test)]
#[path = "durable_state_change_test.rs"]
mod tests;

use alloc::string::String;

/// Durable state change returned by update queries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableStateChange<A> {
  offset:         usize,
  persistence_id: String,
  revision:       u64,
  tag:            String,
  value:          A,
}

impl<A> DurableStateChange<A> {
  /// Creates a durable state change record.
  #[must_use]
  pub const fn new(offset: usize, persistence_id: String, revision: u64, tag: String, value: A) -> Self {
    Self { offset, persistence_id, revision, tag, value }
  }

  /// Returns the change offset.
  #[must_use]
  pub const fn offset(&self) -> usize {
    self.offset
  }

  /// Returns the persistence identifier.
  #[must_use]
  pub fn persistence_id(&self) -> &str {
    &self.persistence_id
  }

  /// Returns the stored revision after the change.
  #[must_use]
  pub const fn revision(&self) -> u64 {
    self.revision
  }

  /// Returns the durable state tag.
  #[must_use]
  pub fn tag(&self) -> &str {
    &self.tag
  }

  /// Returns the changed value.
  #[must_use]
  pub const fn value(&self) -> &A {
    &self.value
  }

  /// Consumes the change and returns the changed value.
  #[must_use]
  pub fn into_value(self) -> A {
    self.value
  }
}
