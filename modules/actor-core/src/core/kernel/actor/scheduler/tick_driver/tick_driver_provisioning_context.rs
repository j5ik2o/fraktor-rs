//! Immutable view used while provisioning tick drivers.

use crate::core::kernel::{
  actor::scheduler::{SchedulerBackedDelayProvider, SchedulerContext, SchedulerShared},
  event::stream::EventStreamShared,
};

/// Immutable context used while provisioning a tick driver.
pub struct TickDriverProvisioningContext {
  scheduler:      SchedulerShared,
  delay_provider: SchedulerBackedDelayProvider,
  event_stream:   EventStreamShared,
}

impl TickDriverProvisioningContext {
  /// Creates a new provisioning context from scheduler handles.
  #[must_use]
  pub const fn new(
    scheduler: SchedulerShared,
    delay_provider: SchedulerBackedDelayProvider,
    event_stream: EventStreamShared,
  ) -> Self {
    Self { scheduler, delay_provider, event_stream }
  }

  /// Builds a provisioning context from a scheduler context.
  #[must_use]
  pub fn from_scheduler_context(context: &SchedulerContext) -> Self {
    Self::new(context.scheduler(), context.delay_provider(), context.event_stream())
  }

  /// Returns the shared scheduler handle.
  #[must_use]
  pub fn scheduler(&self) -> SchedulerShared {
    self.scheduler.clone()
  }

  /// Returns the delay provider connected to the scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider {
    self.delay_provider.clone()
  }

  /// Returns the event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> EventStreamShared {
    self.event_stream.clone()
  }
}
