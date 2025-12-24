//! Immutable view used while provisioning tick drivers.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  dispatch::scheduler::{SchedulerBackedDelayProvider, SchedulerContext, SchedulerSharedGeneric},
  event_stream::EventStreamSharedGeneric,
};

/// Immutable context used while provisioning a tick driver.
pub struct TickDriverProvisioningContext<TB: RuntimeToolbox + 'static> {
  scheduler:      SchedulerSharedGeneric<TB>,
  delay_provider: SchedulerBackedDelayProvider<TB>,
  event_stream:   EventStreamSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> TickDriverProvisioningContext<TB> {
  /// Creates a new provisioning context from scheduler handles.
  #[must_use]
  pub const fn new(
    scheduler: SchedulerSharedGeneric<TB>,
    delay_provider: SchedulerBackedDelayProvider<TB>,
    event_stream: EventStreamSharedGeneric<TB>,
  ) -> Self {
    Self { scheduler, delay_provider, event_stream }
  }

  /// Builds a provisioning context from a scheduler context.
  #[must_use]
  pub fn from_scheduler_context(context: &SchedulerContext<TB>) -> Self {
    Self::new(context.scheduler(), context.delay_provider(), context.event_stream())
  }

  /// Returns the shared scheduler handle.
  #[must_use]
  pub fn scheduler(&self) -> SchedulerSharedGeneric<TB> {
    self.scheduler.clone()
  }

  /// Returns the delay provider connected to the scheduler.
  #[must_use]
  pub fn delay_provider(&self) -> SchedulerBackedDelayProvider<TB> {
    self.delay_provider.clone()
  }

  /// Returns the event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> EventStreamSharedGeneric<TB> {
    self.event_stream.clone()
  }
}
