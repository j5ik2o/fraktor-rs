//! Shared wrapper for `GrainMetrics`.

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::GrainMetrics;

/// Shared wrapper enabling interior mutability for [`GrainMetrics`].
pub struct GrainMetricsSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<GrainMetrics, TB>>,
}

impl<TB: RuntimeToolbox + 'static> GrainMetricsSharedGeneric<TB> {
  /// Creates a new shared wrapper around grain metrics.
  #[must_use]
  pub fn new(metrics: GrainMetrics) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(metrics)) }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<ToolboxMutex<GrainMetrics, TB>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for GrainMetricsSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<GrainMetrics> for GrainMetricsSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&GrainMetrics) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut GrainMetrics) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
