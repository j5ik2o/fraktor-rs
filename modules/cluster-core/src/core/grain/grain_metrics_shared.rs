//! Shared wrapper for `GrainMetrics`.

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::GrainMetrics;

/// Shared wrapper enabling interior mutability for [`GrainMetrics`].
pub struct GrainMetricsShared {
  inner: ArcShared<RuntimeMutex<GrainMetrics>>,
}

impl GrainMetricsShared {
  /// Creates a new shared wrapper around grain metrics.
  #[must_use]
  pub fn new(metrics: GrainMetrics) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(metrics)) }
  }
}

impl Clone for GrainMetricsShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<GrainMetrics> for GrainMetricsShared {
  fn with_read<R>(&self, f: impl FnOnce(&GrainMetrics) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut GrainMetrics) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
