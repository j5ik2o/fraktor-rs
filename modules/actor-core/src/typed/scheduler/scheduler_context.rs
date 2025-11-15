use fraktor_utils_core_rs::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use super::TypedSchedulerShared;
use crate::{
  scheduler::{SchedulerBackedDelayProvider, SchedulerConfig, SchedulerContext, TaskRunSummary},
  typed::TypedScheduler,
};

/// Owns the shared scheduler instance and exposes auxiliary services.
pub struct TypedSchedulerContext<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<SchedulerContext<TB>>,
}

impl<TB: RuntimeToolbox + 'static> TypedSchedulerContext<TB> {
  /// Creates a service from the provided toolbox and configuration.
  #[must_use]
  pub fn new_with_config(toolbox: TB, config: SchedulerConfig) -> Self {
    Self::new(SchedulerContext::new(toolbox, config))
  }

  /// Creates a service from the provided scheduler instance.
  #[must_use]
  pub fn new(inner: SchedulerContext<TB>) -> Self {
    Self::from_shared(ArcShared::new(inner))
  }

  /// Wraps an `ArcShared` pointing at the canonical scheduler context.
  #[must_use]
  pub const fn from_shared(inner: ArcShared<SchedulerContext<TB>>) -> Self {
    Self { inner }
  }

  /// Returns a typed view of the shared scheduler mutex.
  #[must_use]
  pub fn scheduler(&self) -> TypedSchedulerShared<TB> {
    TypedSchedulerShared::new(self.inner.scheduler())
  }

  /// Executes the provided closure while holding the scheduler lock.
  pub fn with_scheduler<F, R>(&self, callback: F) -> R
  where
    F: for<'a> FnOnce(&mut TypedScheduler<'a, TB>) -> R, {
    let shared = self.scheduler();
    let mut guard = shared.lock();
    guard.with(callback)
  }

  /// Returns a delay provider connected to this scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider<TB> {
    self.inner.delay_provider()
  }

  /// Shuts down the underlying scheduler, returning the summary.
  #[must_use]
  pub fn shutdown(&self) -> TaskRunSummary {
    self.inner.shutdown()
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for TypedSchedulerContext<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
