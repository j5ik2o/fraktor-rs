#[cfg(test)]
mod tests;

const DEFAULT_BUFFER_CAPACITY: usize = 32;
const DEFAULT_DEMAND_REDELIVERY_INTERVAL_TICKS: u32 = 1;
const DEFAULT_SUBSCRIPTION_TIMEOUT_TICKS: u32 = 30;
const DEFAULT_FINAL_TERMINATION_SIGNAL_DEADLINE_TICKS: u32 = 2;

/// Settings specific to stream references.
///
/// Mirrors Pekko's `StreamRefSettings` as an immutable value object. Duration
/// settings are represented as scheduler ticks to keep `stream-core` independent
/// from runtime-specific time facilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRefSettings {
  buffer_capacity: usize,
  demand_redelivery_interval_ticks: u32,
  subscription_timeout_ticks: u32,
  final_termination_signal_deadline_ticks: u32,
}

impl StreamRefSettings {
  /// Creates stream reference settings with reference defaults.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      buffer_capacity: DEFAULT_BUFFER_CAPACITY,
      demand_redelivery_interval_ticks: DEFAULT_DEMAND_REDELIVERY_INTERVAL_TICKS,
      subscription_timeout_ticks: DEFAULT_SUBSCRIPTION_TIMEOUT_TICKS,
      final_termination_signal_deadline_ticks: DEFAULT_FINAL_TERMINATION_SIGNAL_DEADLINE_TICKS,
    }
  }

  /// Returns the receiver-side eager buffer capacity.
  #[must_use]
  pub const fn buffer_capacity(&self) -> usize {
    self.buffer_capacity
  }

  /// Returns the demand redelivery interval in scheduler ticks.
  #[must_use]
  pub const fn demand_redelivery_interval_ticks(&self) -> u32 {
    self.demand_redelivery_interval_ticks
  }

  /// Returns the remote subscription timeout in scheduler ticks.
  #[must_use]
  pub const fn subscription_timeout_ticks(&self) -> u32 {
    self.subscription_timeout_ticks
  }

  /// Returns the final termination signal deadline in scheduler ticks.
  #[must_use]
  pub const fn final_termination_signal_deadline_ticks(&self) -> u32 {
    self.final_termination_signal_deadline_ticks
  }

  /// Returns a copy with a new receiver-side buffer capacity.
  ///
  /// # Panics
  ///
  /// Panics when `buffer_capacity` is zero.
  #[must_use]
  pub const fn with_buffer_capacity(mut self, buffer_capacity: usize) -> Self {
    assert!(buffer_capacity > 0, "stream ref buffer capacity must be greater than zero");
    self.buffer_capacity = buffer_capacity;
    self
  }

  /// Returns a copy with a new demand redelivery interval.
  #[must_use]
  pub const fn with_demand_redelivery_interval_ticks(mut self, demand_redelivery_interval_ticks: u32) -> Self {
    self.demand_redelivery_interval_ticks = demand_redelivery_interval_ticks;
    self
  }

  /// Returns a copy with a new remote subscription timeout.
  #[must_use]
  pub const fn with_subscription_timeout_ticks(mut self, subscription_timeout_ticks: u32) -> Self {
    self.subscription_timeout_ticks = subscription_timeout_ticks;
    self
  }

  /// Returns a copy with a new final termination signal deadline.
  #[must_use]
  pub const fn with_termination_received_before_completion_leeway_ticks(
    mut self,
    final_termination_signal_deadline_ticks: u32,
  ) -> Self {
    self.final_termination_signal_deadline_ticks = final_termination_signal_deadline_ticks;
    self
  }
}

impl Default for StreamRefSettings {
  fn default() -> Self {
    Self::new()
  }
}
