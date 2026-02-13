//! Scheduler runtime container used across the actor system.
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxRwLock, sync_rwlock_family::SyncRwLockFamily},
  sync::{ArcShared, SharedAccess},
};

use super::{
  Scheduler, SchedulerBackedDelayProvider, SchedulerConfig, SchedulerSharedGeneric, task_run::TaskRunSummary,
};
use crate::core::event::stream::EventStreamSharedGeneric;

/// Owns the shared scheduler instance and exposes auxiliary services.
pub struct SchedulerContext<TB: RuntimeToolbox + 'static> {
  scheduler:    SchedulerSharedGeneric<TB>,
  provider:     SchedulerBackedDelayProvider<TB>,
  event_stream: EventStreamSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> SchedulerContext<TB> {
  /// Creates a service from the provided toolbox and configuration.
  #[must_use]
  pub fn new(toolbox: TB, config: SchedulerConfig) -> Self {
    Self::with_event_stream(toolbox, config, EventStreamSharedGeneric::default())
  }

  /// Creates a service with the specified event stream handle.
  #[must_use]
  pub fn with_event_stream(toolbox: TB, config: SchedulerConfig, event_stream: EventStreamSharedGeneric<TB>) -> Self {
    let scheduler = Scheduler::new(toolbox, config);
    let rwlock: ToolboxRwLock<_, TB> = <<TB as RuntimeToolbox>::RwLockFamily as SyncRwLockFamily>::create(scheduler);
    let shared = SchedulerSharedGeneric::new(ArcShared::new(rwlock));
    let provider = SchedulerBackedDelayProvider::new(shared.clone());
    Self { scheduler: shared, provider, event_stream }
  }

  /// Returns a clone of the shared scheduler mutex.
  #[must_use]
  pub fn scheduler(&self) -> SchedulerSharedGeneric<TB> {
    self.scheduler.clone()
  }

  /// Returns a delay provider connected to this scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider<TB> {
    self.provider.clone()
  }

  /// Returns the event stream associated with this scheduler.
  #[must_use]
  pub fn event_stream(&self) -> EventStreamSharedGeneric<TB> {
    self.event_stream.clone()
  }

  /// Shuts down the underlying scheduler, returning the summary.
  #[must_use]
  pub fn shutdown(&mut self) -> TaskRunSummary {
    self.scheduler.with_write(|s| s.shutdown_with_tasks())
  }
}
