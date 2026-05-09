//! Grain metrics state for call/activation observability.

use super::GrainMetricsSnapshot;

/// Metrics state when enabled.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GrainMetrics {
  call_failures:          u64,
  call_timeouts:          u64,
  call_retries:           u64,
  activations_created:    u64,
  activations_passivated: u64,
}

impl GrainMetrics {
  /// Creates empty metrics.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      call_failures:          0,
      call_timeouts:          0,
      call_retries:           0,
      activations_created:    0,
      activations_passivated: 0,
    }
  }

  /// Records a call failure.
  pub const fn record_call_failed(&mut self) {
    self.call_failures = self.call_failures.saturating_add(1);
  }

  /// Records a call timeout.
  pub const fn record_call_timed_out(&mut self) {
    self.call_timeouts = self.call_timeouts.saturating_add(1);
  }

  /// Records a call retry.
  pub const fn record_call_retried(&mut self) {
    self.call_retries = self.call_retries.saturating_add(1);
  }

  /// Records an activation creation.
  pub const fn record_activation_created(&mut self) {
    self.activations_created = self.activations_created.saturating_add(1);
  }

  /// Records an activation passivation.
  pub const fn record_activation_passivated(&mut self) {
    self.activations_passivated = self.activations_passivated.saturating_add(1);
  }

  /// Returns a snapshot of the current metrics.
  #[must_use]
  pub const fn snapshot(&self) -> GrainMetricsSnapshot {
    GrainMetricsSnapshot::new(
      self.call_failures,
      self.call_timeouts,
      self.call_retries,
      self.activations_created,
      self.activations_passivated,
    )
  }
}
