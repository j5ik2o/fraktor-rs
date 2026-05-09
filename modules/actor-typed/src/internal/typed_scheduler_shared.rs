use fraktor_actor_core_kernel_rs::actor::scheduler::SchedulerShared;
use fraktor_utils_core_rs::core::sync::SharedAccess;

use super::TypedSchedulerGuard;

/// Shared handle that provides typed access to the scheduler mutex.
#[derive(Clone)]
pub struct TypedSchedulerShared {
  inner: SchedulerShared,
}

impl TypedSchedulerShared {
  /// Builds the typed view from the canonical scheduler handle.
  #[must_use]
  pub const fn new(inner: SchedulerShared) -> Self {
    Self { inner }
  }

  /// Executes a closure while holding the scheduler lock, exposing a typed guard.
  pub fn with_write<R>(&self, f: impl FnOnce(&mut TypedSchedulerGuard<'_>) -> R) -> R {
    self.inner.with_write(|scheduler| {
      let mut guard = TypedSchedulerGuard::new(scheduler);
      f(&mut guard)
    })
  }

  /// Returns the underlying shared handle (for wiring).
  #[must_use]
  pub fn raw(&self) -> SchedulerShared {
    self.inner.clone()
  }
}
