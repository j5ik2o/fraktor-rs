//! Scheduler runtime container used across the actor system.
use fraktor_utils_core_rs::core::sync::{DefaultRwLock, SharedAccess, SharedRwLock};

use super::{Scheduler, SchedulerBackedDelayProvider, SchedulerConfig, SchedulerShared, task_run::TaskRunSummary};
use crate::event::stream::EventStreamShared;

/// Owns the shared scheduler instance and exposes auxiliary services.
pub struct SchedulerContext {
  scheduler:    SchedulerShared,
  provider:     SchedulerBackedDelayProvider,
  event_stream: EventStreamShared,
}

impl SchedulerContext {
  /// Creates a service from the provided configuration.
  #[must_use]
  pub fn new(config: SchedulerConfig) -> Self {
    Self::with_event_stream(config, EventStreamShared::default())
  }

  /// Creates a service with the specified event stream handle.
  #[must_use]
  pub fn with_event_stream(config: SchedulerConfig, event_stream: EventStreamShared) -> Self {
    let scheduler = Scheduler::new(config);
    let shared = SchedulerShared::new(SharedRwLock::new_with_driver::<DefaultRwLock<_>>(scheduler));
    let provider = SchedulerBackedDelayProvider::new(shared.clone());
    Self { scheduler: shared, provider, event_stream }
  }

  /// Returns a clone of the shared scheduler mutex.
  #[must_use]
  pub fn scheduler(&self) -> SchedulerShared {
    self.scheduler.clone()
  }

  /// Returns a delay provider connected to this scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider {
    self.provider.clone()
  }

  /// Returns the event stream associated with this scheduler.
  #[must_use]
  pub fn event_stream(&self) -> EventStreamShared {
    self.event_stream.clone()
  }

  /// Shuts down the underlying scheduler, returning the summary.
  #[must_use]
  pub fn shutdown(&mut self) -> TaskRunSummary {
    self.scheduler.with_write(|s| s.shutdown_with_tasks())
  }
}
