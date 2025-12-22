use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::SharedAccess};

use super::TypedSchedulerShared;
use crate::core::{
  scheduler::{
    SchedulerBackedDelayProvider, SchedulerConfig, SchedulerContext, SchedulerSharedGeneric, TaskRunSummary,
  },
  typed::TypedScheduler,
};

/// Owns the shared scheduler instance and exposes auxiliary services.
pub struct TypedSchedulerContext<TB: RuntimeToolbox + 'static> {
  scheduler:      SchedulerSharedGeneric<TB>,
  delay_provider: SchedulerBackedDelayProvider<TB>,
}

impl<TB: RuntimeToolbox + 'static> TypedSchedulerContext<TB> {
  /// Creates a service from the provided toolbox and configuration.
  #[must_use]
  pub fn new_with_config(toolbox: TB, config: SchedulerConfig) -> Self {
    let context = SchedulerContext::new(toolbox, config);
    Self::new(&context)
  }

  /// Creates a service from the provided scheduler instance.
  #[must_use]
  pub fn new(inner: &SchedulerContext<TB>) -> Self {
    Self::from_handles(inner.scheduler(), inner.delay_provider())
  }

  /// Wraps scheduler handles.
  #[must_use]
  pub const fn from_handles(
    scheduler: SchedulerSharedGeneric<TB>,
    delay_provider: SchedulerBackedDelayProvider<TB>,
  ) -> Self {
    Self { scheduler, delay_provider }
  }

  /// Returns a typed view of the shared scheduler mutex.
  #[must_use]
  pub fn scheduler(&self) -> TypedSchedulerShared<TB> {
    TypedSchedulerShared::new(self.scheduler.clone())
  }

  /// Executes the provided closure while holding the scheduler lock.
  pub fn with_scheduler<F, R>(&self, callback: F) -> R
  where
    F: for<'a> FnOnce(&mut TypedScheduler<'a, TB>) -> R, {
    let shared = self.scheduler();
    shared.with_write(|guard| guard.with(callback))
  }

  /// Returns a delay provider connected to this scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider<TB> {
    self.delay_provider.clone()
  }

  /// Shuts down the underlying scheduler, returning the summary.
  #[must_use]
  pub fn shutdown(&mut self) -> TaskRunSummary {
    self.scheduler.with_write(|s| s.shutdown_with_tasks())
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for TypedSchedulerContext<TB> {
  fn clone(&self) -> Self {
    Self { scheduler: self.scheduler.clone(), delay_provider: self.delay_provider.clone() }
  }
}
