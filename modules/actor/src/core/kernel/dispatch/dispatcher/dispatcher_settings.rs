use core::time::Duration;

use super::{inline_schedule_adapter::InlineScheduleAdapter, schedule_adapter_shared::ScheduleAdapterShared};

/// Immutable settings snapshot passed to dispatcher providers.
#[derive(Clone)]
pub struct DispatcherSettings {
  throughput_deadline: Option<Duration>,
  starvation_deadline: Option<Duration>,
  schedule_adapter:    ScheduleAdapterShared,
}

impl DispatcherSettings {
  /// Creates settings with the provided schedule adapter.
  #[must_use]
  pub const fn new(schedule_adapter: ScheduleAdapterShared) -> Self {
    Self { throughput_deadline: None, starvation_deadline: None, schedule_adapter }
  }

  /// Returns the configured throughput deadline.
  #[must_use]
  pub const fn throughput_deadline(&self) -> Option<Duration> {
    self.throughput_deadline
  }

  /// Returns the configured starvation deadline.
  #[must_use]
  pub const fn starvation_deadline(&self) -> Option<Duration> {
    self.starvation_deadline
  }

  /// Overrides the throughput deadline.
  #[must_use]
  pub const fn with_throughput_deadline(mut self, deadline: Option<Duration>) -> Self {
    self.throughput_deadline = deadline;
    self
  }

  /// Overrides the starvation deadline.
  #[must_use]
  pub const fn with_starvation_deadline(mut self, deadline: Option<Duration>) -> Self {
    self.starvation_deadline = deadline;
    self
  }

  /// Overrides both deadlines.
  #[must_use]
  pub const fn with_deadlines(mut self, throughput: Option<Duration>, starvation: Option<Duration>) -> Self {
    self.throughput_deadline = throughput;
    self.starvation_deadline = starvation;
    self
  }

  /// Overrides the schedule adapter snapshot.
  #[must_use]
  pub fn with_schedule_adapter(mut self, adapter: ScheduleAdapterShared) -> Self {
    self.schedule_adapter = adapter;
    self
  }

  /// Returns the configured schedule adapter snapshot.
  #[must_use]
  pub fn schedule_adapter(&self) -> ScheduleAdapterShared {
    self.schedule_adapter.clone()
  }
}

impl Default for DispatcherSettings {
  fn default() -> Self {
    Self::new(InlineScheduleAdapter::shared())
  }
}
