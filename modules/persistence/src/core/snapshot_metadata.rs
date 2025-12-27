//! Snapshot metadata representation.

use alloc::string::String;
use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

/// Metadata associated with a stored snapshot.
pub struct SnapshotMetadata {
  persistence_id: String,
  sequence_nr:    u64,
  timestamp:      u64,
  metadata:       Option<ArcShared<dyn Any + Send + Sync>>,
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

  /// Returns the optional metadata payload.
  #[must_use]
  pub fn metadata(&self) -> Option<&(dyn Any + Send + Sync)> {
    self.metadata.as_deref()
  }

  /// Returns a clone of the metadata payload.
  #[must_use]
  pub fn metadata_arc(&self) -> Option<ArcShared<dyn Any + Send + Sync>> {
    self.metadata.clone()
  }

  /// Returns a copy with attached metadata.
  #[must_use]
  pub fn with_metadata(mut self, metadata: ArcShared<dyn Any + Send + Sync>) -> Self {
    self.metadata = Some(metadata);
    self
  }
}

impl Clone for SnapshotMetadata {
  fn clone(&self) -> Self {
    Self {
      persistence_id: self.persistence_id.clone(),
      sequence_nr:    self.sequence_nr,
      timestamp:      self.timestamp,
      metadata:       self.metadata.clone(),
    }
  }
}

impl core::fmt::Debug for SnapshotMetadata {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("SnapshotMetadata")
      .field("persistence_id", &self.persistence_id)
      .field("sequence_nr", &self.sequence_nr)
      .field("timestamp", &self.timestamp)
      .field("has_metadata", &self.metadata.is_some())
      .finish()
  }
}

impl PartialEq for SnapshotMetadata {
  fn eq(&self, other: &Self) -> bool {
    self.persistence_id == other.persistence_id
      && self.sequence_nr == other.sequence_nr
      && self.timestamp == other.timestamp
  }
}

impl Eq for SnapshotMetadata {}
