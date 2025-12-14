//! Scheduler runtime container shared across the actor system.
use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncRwLockFamily, ToolboxRwLock},
  sync::{ArcShared, SharedAccess},
};

use super::{
  Scheduler, SchedulerBackedDelayProvider, SchedulerConfig, SchedulerSharedGeneric, TaskRunSummary,
  tick_driver::{AutoDriverMetadata, TickDriverMetadata},
};
use crate::core::event_stream::{EventStreamSharedGeneric, TickDriverSnapshot};

/// Owns the shared scheduler instance and exposes auxiliary services.
pub struct SchedulerContext<TB: RuntimeToolbox + 'static> {
  scheduler:       SchedulerSharedGeneric<TB>,
  provider:        SchedulerBackedDelayProvider<TB>,
  event_stream:    EventStreamSharedGeneric<TB>,
  driver_snapshot: Option<TickDriverSnapshot>,
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
    Self { scheduler: shared, provider, event_stream, driver_snapshot: None }
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

  /// Returns the last recorded driver metadata.
  #[must_use]
  pub fn driver_metadata(&self) -> Option<TickDriverMetadata> {
    self.driver_snapshot.as_ref().map(|snapshot| snapshot.metadata.clone())
  }

  /// Returns the last recorded auto driver metadata.
  #[must_use]
  pub fn auto_driver_metadata(&self) -> Option<AutoDriverMetadata> {
    self.driver_snapshot.as_ref().and_then(|snapshot| snapshot.auto.clone())
  }

  /// Returns the last published driver snapshot.
  #[must_use]
  pub fn driver_snapshot(&self) -> Option<TickDriverSnapshot> {
    self.driver_snapshot.clone()
  }

  /// Shuts down the underlying scheduler, returning the summary.
  #[must_use]
  pub fn shutdown(&mut self) -> TaskRunSummary {
    self.scheduler.with_write(|s| s.shutdown_with_tasks())
  }
}
