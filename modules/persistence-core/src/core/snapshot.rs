//! Snapshot payload container.

#[cfg(test)]
mod tests;

use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::snapshot_metadata::SnapshotMetadata;

/// Snapshot data with metadata.
#[derive(Clone, Debug)]
pub struct Snapshot {
  metadata: SnapshotMetadata,
  data:     ArcShared<dyn Any + Send + Sync>,
}

impl Snapshot {
  /// Creates a new snapshot.
  #[must_use]
  pub fn new(metadata: SnapshotMetadata, data: ArcShared<dyn Any + Send + Sync>) -> Self {
    Self { metadata, data }
  }

  /// Returns the snapshot metadata.
  #[must_use]
  pub const fn metadata(&self) -> &SnapshotMetadata {
    &self.metadata
  }

  /// Returns the raw snapshot data.
  #[must_use]
  pub fn data(&self) -> &ArcShared<dyn Any + Send + Sync> {
    &self.data
  }

  /// Attempts to downcast the snapshot payload.
  #[must_use]
  pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
    self.data.downcast_ref::<T>()
  }
}
