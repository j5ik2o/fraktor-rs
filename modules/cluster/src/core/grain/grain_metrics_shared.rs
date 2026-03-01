//! Shared wrapper for `GrainMetrics`.

use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeMutex, RuntimeToolbox},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::GrainMetrics;

/// Shared wrapper enabling interior mutability for [`GrainMetrics`].
pub struct GrainMetricsSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner:   ArcShared<RuntimeMutex<GrainMetrics>>,
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> GrainMetricsSharedGeneric<TB> {
  /// Creates a new shared wrapper around grain metrics.
  #[must_use]
  pub fn new(metrics: GrainMetrics) -> Self {
    Self { inner: ArcShared::new(RuntimeMutex::new(metrics)), _marker: PhantomData }
  }

  /// Returns a cloned handle to the inner shared mutex.
  #[must_use]
  pub fn inner(&self) -> ArcShared<RuntimeMutex<GrainMetrics>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for GrainMetricsSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), _marker: PhantomData }
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
