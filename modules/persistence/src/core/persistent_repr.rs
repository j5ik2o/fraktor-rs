//! Persistent event representation stored by journals.

use alloc::string::String;
use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

/// Persistent event stored in the journal.
pub struct PersistentRepr {
  payload:        ArcShared<dyn Any + Send + Sync>,
  persistence_id: String,
  sequence_nr:    u64,
  manifest:       String,
  timestamp:      u64,
  writer_id:      String,
  metadata:       Option<ArcShared<dyn Any + Send + Sync>>,
}

impl PersistentRepr {
  /// Creates a new persistent representation from a payload.
  #[must_use]
  pub fn from_payload<E>(
    payload: E,
    persistence_id: impl Into<String>,
    sequence_nr: u64,
    timestamp: u64,
    writer_id: impl Into<String>,
  ) -> Self
  where
    E: Any + Send + Sync + 'static, {
    Self {
      payload: ArcShared::new(payload),
      persistence_id: persistence_id.into(),
      sequence_nr,
      manifest: String::new(),
      timestamp,
      writer_id: writer_id.into(),
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

  /// Returns the event manifest.
  #[must_use]
  pub fn manifest(&self) -> &str {
    &self.manifest
  }

  /// Returns the timestamp.
  #[must_use]
  pub const fn timestamp(&self) -> u64 {
    self.timestamp
  }

  /// Returns the writer identifier.
  #[must_use]
  pub fn writer_id(&self) -> &str {
    &self.writer_id
  }

  /// Returns the metadata payload.
  #[must_use]
  pub fn metadata(&self) -> Option<&(dyn Any + Send + Sync)> {
    self.metadata.as_deref()
  }

  /// Returns the event payload.
  #[must_use]
  pub fn payload(&self) -> &(dyn Any + Send + Sync) {
    &*self.payload
  }

  /// Returns a clone of the payload pointer.
  #[must_use]
  pub fn payload_arc(&self) -> ArcShared<dyn Any + Send + Sync> {
    self.payload.clone()
  }

  /// Attempts to downcast the payload to the requested type.
  #[must_use]
  pub fn downcast_ref<T: Any + Send + Sync + 'static>(&self) -> Option<&T> {
    self.payload.downcast_ref::<T>()
  }

  /// Returns a copy with updated manifest.
  #[must_use]
  pub fn with_manifest(mut self, manifest: impl Into<String>) -> Self {
    self.manifest = manifest.into();
    self
  }

  /// Returns a copy with attached metadata.
  #[must_use]
  pub fn with_metadata(mut self, metadata: ArcShared<dyn Any + Send + Sync>) -> Self {
    self.metadata = Some(metadata);
    self
  }
}

impl Clone for PersistentRepr {
  fn clone(&self) -> Self {
    Self {
      payload:        self.payload.clone(),
      persistence_id: self.persistence_id.clone(),
      sequence_nr:    self.sequence_nr,
      manifest:       self.manifest.clone(),
      timestamp:      self.timestamp,
      writer_id:      self.writer_id.clone(),
      metadata:       self.metadata.clone(),
    }
  }
}

impl core::fmt::Debug for PersistentRepr {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("PersistentRepr")
      .field("persistence_id", &self.persistence_id)
      .field("sequence_nr", &self.sequence_nr)
      .field("manifest", &self.manifest)
      .field("timestamp", &self.timestamp)
      .field("writer_id", &self.writer_id)
      .field("has_metadata", &self.metadata.is_some())
      .finish()
  }
}
