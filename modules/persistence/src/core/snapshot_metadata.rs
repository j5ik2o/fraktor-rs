//! Snapshot metadata representation.

#[cfg(test)]
mod tests;

use alloc::string::String;

/// Metadata describing a stored snapshot.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SnapshotMetadata {
  persistence_id: String,
  sequence_nr:    u64,
  timestamp:      u64,
  metadata:       Option<String>,
}

impl SnapshotMetadata {
  /// Creates new snapshot metadata.
  #[must_use]
  pub fn new(persistence_id: impl Into<String>, sequence_nr: u64, timestamp: u64) -> Self {
    Self { persistence_id: persistence_id.into(), sequence_nr, timestamp, metadata: None }
  }

  /// Returns the persistence id.
  #[must_use]
  pub fn persistence_id(&self) -> &str {
    &self.persistence_id
  }

  /// Returns the sequence number.
  #[must_use]
  pub const fn sequence_nr(&self) -> u64 {
    self.sequence_nr
  }

  /// Returns the timestamp.
  #[must_use]
  pub const fn timestamp(&self) -> u64 {
    self.timestamp
  }

  /// Returns the extra metadata if present.
  #[must_use]
  pub fn metadata(&self) -> Option<&str> {
    self.metadata.as_deref()
  }

  /// Adds extra metadata to this snapshot metadata.
  #[must_use]
  pub fn with_metadata(mut self, metadata: impl Into<String>) -> Self {
    self.metadata = Some(metadata.into());
    self
  }
}
