use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::typed_scheduler_guard::TypedSchedulerGuard;
use crate::core::scheduler::Scheduler;

/// Shared handle that provides typed access to the scheduler mutex.
pub struct TypedSchedulerShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<Scheduler<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> TypedSchedulerShared<TB> {
  /// Builds the typed view from the canonical scheduler handle.
  #[must_use]
  pub const fn new(inner: ArcShared<ToolboxMutex<Scheduler<TB>, TB>>) -> Self {
    Self { inner }
  }

  /// Locks the underlying scheduler mutex and returns a typed guard.
  #[must_use]
  pub fn lock(&self) -> TypedSchedulerGuard<'_, TB> {
    TypedSchedulerGuard { guard: self.inner.lock() }
  }

  /// Returns the underlying shared mutex in case callers need raw access.
  #[must_use]
  pub fn raw(&self) -> ArcShared<ToolboxMutex<Scheduler<TB>, TB>> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for TypedSchedulerShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
