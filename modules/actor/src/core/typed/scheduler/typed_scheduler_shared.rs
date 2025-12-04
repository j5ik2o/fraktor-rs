use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::typed_scheduler_guard::TypedSchedulerGuard;
use crate::core::scheduler::SchedulerSharedGeneric;

/// Shared handle that provides typed access to the scheduler mutex.
pub struct TypedSchedulerShared<TB: RuntimeToolbox + 'static> {
  inner: SchedulerSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> TypedSchedulerShared<TB> {
  /// Builds the typed view from the canonical scheduler handle.
  #[must_use]
  pub const fn new(inner: SchedulerSharedGeneric<TB>) -> Self {
    Self { inner }
  }

  /// Executes a closure while holding the scheduler lock, exposing a typed guard.
  pub fn with_mut<R>(&self, f: impl FnOnce(&mut TypedSchedulerGuard<'_, TB>) -> R) -> R {
    let mut guard = TypedSchedulerGuard { guard: self.inner.lock_guard() };
    f(&mut guard)
  }

  /// Returns the underlying shared handle (for wiring).
  #[must_use]
  pub fn raw(&self) -> SchedulerSharedGeneric<TB> {
    self.inner.clone()
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for TypedSchedulerShared<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
