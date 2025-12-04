use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::TypedSchedulerShared;
use crate::core::{
  scheduler::{
    SchedulerBackedDelayProvider, SchedulerConfig, SchedulerContext, SchedulerContextSharedGeneric, TaskRunSummary,
  },
  typed::TypedScheduler,
};

/// Owns the shared scheduler instance and exposes auxiliary services.
pub struct TypedSchedulerContext<TB: RuntimeToolbox + 'static> {
  inner: SchedulerContextSharedGeneric<TB>,
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
    Self::from_shared(SchedulerContextSharedGeneric::new(inner))
  }

  /// Wraps a shared context.
  #[must_use]
  pub const fn from_shared(inner: SchedulerContextSharedGeneric<TB>) -> Self {
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
    shared.with_mut(|guard| guard.with(callback))
  }

  /// Returns a delay provider connected to this scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider<TB> {
    self.inner.delay_provider()
  }

  /// Shuts down the underlying scheduler, returning the summary.
  #[must_use]
  pub fn shutdown(&mut self) -> TaskRunSummary {
    self.inner.shutdown()
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for TypedSchedulerContext<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}
