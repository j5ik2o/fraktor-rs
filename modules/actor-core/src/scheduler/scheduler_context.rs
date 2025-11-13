//! Scheduler runtime container shared across the actor system.

use fraktor_utils_core_rs::{
  runtime_toolbox::SyncMutexFamily,
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::{Scheduler, SchedulerBackedDelayProvider, SchedulerConfig, TaskRunSummary};
use crate::{RuntimeToolbox, ToolboxMutex};

/// Owns the shared scheduler instance and exposes auxiliary services.
pub struct SchedulerContext<TB: RuntimeToolbox + 'static> {
  scheduler: ArcShared<ToolboxMutex<Scheduler<TB>, TB>>,
  provider:  SchedulerBackedDelayProvider<TB>,
}

impl<TB: RuntimeToolbox + 'static> SchedulerContext<TB> {
  /// Creates a service from the provided toolbox and configuration.
  #[must_use]
  pub fn new(toolbox: TB, config: SchedulerConfig) -> Self {
    let scheduler = Scheduler::new(toolbox, config);
    let mutex = <<TB as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(scheduler);
    let shared = ArcShared::new(mutex);
    let provider = SchedulerBackedDelayProvider::new(shared.clone());
    Self { scheduler: shared, provider }
  }

  /// Returns a clone of the shared scheduler mutex.
  #[must_use]
  pub fn scheduler(&self) -> ArcShared<ToolboxMutex<Scheduler<TB>, TB>> {
    self.scheduler.clone()
  }

  /// Returns a delay provider connected to this scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider<TB> {
    self.provider.clone()
  }

  /// Shuts down the underlying scheduler, returning the summary.
  pub fn shutdown(&self) -> TaskRunSummary {
    let scheduler = self.scheduler.clone();
    let mut guard = scheduler.lock();
    guard.shutdown_with_tasks()
  }
}
