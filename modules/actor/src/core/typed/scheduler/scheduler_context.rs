use fraktor_utils_rs::core::sync::SharedAccess;

use super::TypedSchedulerShared;
use crate::core::{
  kernel::scheduler::{
    SchedulerBackedDelayProvider, SchedulerConfig, SchedulerContext, SchedulerShared, task_run::TaskRunSummary,
  },
  typed::scheduler::TypedScheduler,
};

/// Owns the shared scheduler instance and exposes auxiliary services.
pub struct TypedSchedulerContext {
  scheduler:      SchedulerShared,
  delay_provider: SchedulerBackedDelayProvider,
}

impl TypedSchedulerContext {
  /// Creates a service from the provided configuration.
  #[must_use]
  pub fn new_with_config(config: SchedulerConfig) -> Self {
    let context = SchedulerContext::new(config);
    Self::new(&context)
  }

  /// Creates a service from the provided scheduler instance.
  #[must_use]
  pub fn new(inner: &SchedulerContext) -> Self {
    Self::from_handles(inner.scheduler(), inner.delay_provider())
  }

  /// Wraps scheduler handles.
  #[must_use]
  pub const fn from_handles(scheduler: SchedulerShared, delay_provider: SchedulerBackedDelayProvider) -> Self {
    Self { scheduler, delay_provider }
  }

  /// Returns a typed view of the shared scheduler mutex.
  #[must_use]
  pub fn scheduler(&self) -> TypedSchedulerShared {
    TypedSchedulerShared::new(self.scheduler.clone())
  }

  /// Executes the provided closure while holding the scheduler lock.
  pub fn with_scheduler<F, R>(&self, callback: F) -> R
  where
    F: for<'a> FnOnce(&mut TypedScheduler<'a>) -> R, {
    let shared = self.scheduler();
    shared.with_write(|guard| guard.with(callback))
  }

  /// Returns a delay provider connected to this scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider {
    self.delay_provider.clone()
  }

  /// Shuts down the underlying scheduler, returning the summary.
  #[must_use]
  pub fn shutdown(&mut self) -> TaskRunSummary {
    self.scheduler.with_write(|s| s.shutdown_with_tasks())
  }
}

impl Clone for TypedSchedulerContext {
  fn clone(&self) -> Self {
    Self { scheduler: self.scheduler.clone(), delay_provider: self.delay_provider.clone() }
  }
}
