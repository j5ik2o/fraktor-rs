//! Shared wrapper for `GrainMetrics`.

use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

use super::GrainMetrics;

/// Shared wrapper enabling interior mutability for [`GrainMetrics`].
pub struct GrainMetricsShared {
  inner: SharedLock<GrainMetrics>,
}

impl GrainMetricsShared {
  /// Creates a new shared wrapper around grain metrics.
  #[must_use]
  pub fn new(metrics: GrainMetrics) -> Self {
    Self { inner: SharedLock::new_with_driver::<DefaultMutex<_>>(metrics) }
  }
}

impl Clone for GrainMetricsShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<GrainMetrics> for GrainMetricsShared {
  fn with_read<R>(&self, f: impl FnOnce(&GrainMetrics) -> R) -> R {
    self.inner.with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut GrainMetrics) -> R) -> R {
    self.inner.with_write(f)
  }
}
