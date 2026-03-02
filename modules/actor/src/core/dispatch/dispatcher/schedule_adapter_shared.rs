//! Shared wrapper for ScheduleAdapter implementations.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeRwLock,
  sync::{ArcShared, SharedAccess, sync_rwlock_like::SyncRwLockLike},
};

use super::schedule_adapter::ScheduleAdapter;

/// Shared wrapper for [`ScheduleAdapter`].
///
/// This wrapper provides [`SharedAccess`] methods (`with_read`/`with_write`)
/// that internally lock the underlying adapter, allowing safe
/// concurrent access from multiple owners.
pub struct ScheduleAdapterShared {
  inner: ArcShared<RuntimeRwLock<Box<dyn ScheduleAdapter>>>,
}

impl ScheduleAdapterShared {
  /// Creates a new shared wrapper around the provided adapter.
  #[must_use]
  pub fn new(adapter: Box<dyn ScheduleAdapter>) -> Self {
    Self { inner: ArcShared::new(RuntimeRwLock::new(adapter)) }
  }
}

impl Clone for ScheduleAdapterShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl SharedAccess<Box<dyn ScheduleAdapter>> for ScheduleAdapterShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ScheduleAdapter>) -> R) -> R {
    let guard = self.inner.read();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ScheduleAdapter>) -> R) -> R {
    let mut guard = self.inner.write();
    f(&mut guard)
  }
}
