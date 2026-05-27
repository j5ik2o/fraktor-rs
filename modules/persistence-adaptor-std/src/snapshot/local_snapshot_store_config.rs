//! Local snapshot store configuration.

#[cfg(test)]
#[path = "local_snapshot_store_config_test.rs"]
mod tests;

use std::path::{Path, PathBuf};

use fraktor_actor_core_kernel_rs::serialization::serialization_registry::SerializationRegistry;
use fraktor_persistence_core_kernel_rs::snapshot::SnapshotError;
use fraktor_utils_core_rs::sync::ArcShared;

const DEFAULT_MAX_LOAD_ATTEMPTS: usize = 3;

/// Configuration for [`LocalSnapshotStore`](super::LocalSnapshotStore).
#[derive(Clone)]
pub struct LocalSnapshotStoreConfig {
  directory:         PathBuf,
  serialization:     ArcShared<SerializationRegistry>,
  max_load_attempts: usize,
}

impl LocalSnapshotStoreConfig {
  /// Creates a new local snapshot store configuration.
  #[must_use]
  pub const fn new(directory: PathBuf, serialization: ArcShared<SerializationRegistry>) -> Self {
    Self { directory, serialization, max_load_attempts: DEFAULT_MAX_LOAD_ATTEMPTS }
  }

  /// Returns the snapshot root directory.
  #[must_use]
  pub fn directory(&self) -> &Path {
    &self.directory
  }

  /// Returns the serialization registry.
  #[must_use]
  pub const fn serialization(&self) -> &ArcShared<SerializationRegistry> {
    &self.serialization
  }

  /// Returns the maximum number of corrupt load candidates to try.
  #[must_use]
  pub const fn max_load_attempts(&self) -> usize {
    self.max_load_attempts
  }

  /// Returns a copy with a different maximum number of load attempts.
  #[must_use]
  pub const fn with_max_load_attempts(mut self, max_load_attempts: usize) -> Self {
    self.max_load_attempts = max_load_attempts;
    self
  }

  /// Validates this configuration.
  ///
  /// # Errors
  ///
  /// Returns [`SnapshotError::LoadFailed`] when `max_load_attempts` is less than two.
  pub fn validate(&self) -> Result<(), SnapshotError> {
    if self.max_load_attempts < 2 {
      return Err(SnapshotError::LoadFailed(String::from(
        "invalid local snapshot store config: max_load_attempts must be at least 2",
      )));
    }
    Ok(())
  }

  pub(crate) fn into_parts(self) -> (PathBuf, ArcShared<SerializationRegistry>, usize) {
    (self.directory, self.serialization, self.max_load_attempts)
  }
}
