//! Shared wrapper for ScheduleAdapter implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use super::schedule_adapter::ScheduleAdapter;

/// Shared wrapper for [`ScheduleAdapter`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying adapter, allowing safe
/// concurrent access from multiple owners.
pub struct ScheduleAdapterSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Box<dyn ScheduleAdapter<TB>>, TB>>,
}

/// Type alias using the default toolbox.
pub type ScheduleAdapterShared = ScheduleAdapterSharedGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> ScheduleAdapterSharedGeneric<TB> {
  /// Creates a new shared wrapper around the provided adapter.
  #[must_use]
  pub fn new(adapter: Box<dyn ScheduleAdapter<TB>>) -> Self {
    Self { inner: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(adapter)) }
  }
}

impl<TB: RuntimeToolbox> Clone for ScheduleAdapterSharedGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> SharedAccess<Box<dyn ScheduleAdapter<TB>>> for ScheduleAdapterSharedGeneric<TB> {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ScheduleAdapter<TB>>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ScheduleAdapter<TB>>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
