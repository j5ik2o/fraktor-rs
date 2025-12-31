//! Persistent event representation.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

/// Persistent event wrapper with metadata.
#[derive(Clone, Debug)]
pub struct PersistentRepr {
  persistence_id: String,
  sequence_nr:    u64,
  payload:        ArcShared<dyn Any + Send + Sync>,
  manifest:       String,
  writer_uuid:    String,
  timestamp:      u64,
  metadata:       Option<ArcShared<dyn Any + Send + Sync>>,
}

impl PersistentRepr {
  /// Creates a new persistent representation.
  #[must_use]
  pub fn new(persistence_id: impl Into<String>, sequence_nr: u64, payload: ArcShared<dyn Any + Send + Sync>) -> Self {
    Self {
      persistence_id: persistence_id.into(),
      sequence_nr,
      payload,
      manifest: String::new(),
      writer_uuid: String::new(),
      timestamp: 0,
      metadata: None,
    }
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

  /// Returns the manifest string.
  #[must_use]
  pub fn manifest(&self) -> &str {
    &self.manifest
  }

  /// Returns the writer uuid string.
  #[must_use]
  pub fn writer_uuid(&self) -> &str {
    &self.writer_uuid
  }

  /// Returns the timestamp.
  #[must_use]
  pub const fn timestamp(&self) -> u64 {
    self.timestamp
  }

  /// Returns the payload.
  #[must_use]
  pub fn payload(&self) -> &ArcShared<dyn Any + Send + Sync> {
    &self.payload
  }

  /// Returns the optional metadata payload.
  #[must_use]
  pub fn metadata(&self) -> Option<&ArcShared<dyn Any + Send + Sync>> {
    self.metadata.as_ref()
  }

  /// Attempts to downcast the payload to the requested type.
  #[must_use]
  pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
    self.payload.downcast_ref::<T>()
  }

  /// Returns a new instance with a different manifest.
  #[must_use]
  pub fn with_manifest(mut self, manifest: impl Into<String>) -> Self {
    self.manifest = manifest.into();
    self
  }

  /// Returns a new instance with attached metadata.
  #[must_use]
  pub fn with_metadata(mut self, metadata: ArcShared<dyn Any + Send + Sync>) -> Self {
    self.metadata = Some(metadata);
    self
  }

  /// Returns a new instance with a different timestamp.
  #[must_use]
  pub const fn with_timestamp(mut self, timestamp: u64) -> Self {
    self.timestamp = timestamp;
    self
  }

  /// Returns a new instance with a different writer uuid.
  #[must_use]
  pub fn with_writer_uuid(mut self, writer_uuid: impl Into<String>) -> Self {
    self.writer_uuid = writer_uuid.into();
    self
  }
}
