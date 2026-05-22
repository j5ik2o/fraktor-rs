//! Snapshot payload wrapper for persistence serialization.

use core::any::Any;

use fraktor_utils_core_rs::sync::ArcShared;

/// Serializable wrapper around snapshot data.
#[derive(Clone, Debug)]
pub struct SnapshotPayload {
  data: ArcShared<dyn Any + Send + Sync>,
}

impl SnapshotPayload {
  /// Creates a new snapshot payload wrapper.
  #[must_use]
  pub const fn new(data: ArcShared<dyn Any + Send + Sync>) -> Self {
    Self { data }
  }

  /// Returns the wrapped snapshot data.
  #[must_use]
  pub const fn data(&self) -> &ArcShared<dyn Any + Send + Sync> {
    &self.data
  }

  /// Attempts to downcast the snapshot data.
  #[must_use]
  pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
    self.data.downcast_ref::<T>()
  }
}
