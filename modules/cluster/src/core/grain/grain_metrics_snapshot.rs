//! Immutable snapshot of collected grain metrics.

/// Read-only grain metrics snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrainMetricsSnapshot {
  call_failures:          u64,
  call_timeouts:          u64,
  call_retries:           u64,
  activations_created:    u64,
  activations_passivated: u64,
}

impl GrainMetricsSnapshot {
  /// Internal constructor used by metrics collector.
  pub(crate) const fn new(
    call_failures: u64,
    call_timeouts: u64,
    call_retries: u64,
    activations_created: u64,
    activations_passivated: u64,
  ) -> Self {
    Self { call_failures, call_timeouts, call_retries, activations_created, activations_passivated }
  }

  /// Failed call count.
  #[must_use]
  pub const fn call_failures(&self) -> u64 {
    self.call_failures
  }

  /// Timeout call count.
  #[must_use]
  pub const fn call_timeouts(&self) -> u64 {
    self.call_timeouts
  }

  /// Retried call count.
  #[must_use]
  pub const fn call_retries(&self) -> u64 {
    self.call_retries
  }

  /// Activation created count.
  #[must_use]
  pub const fn activations_created(&self) -> u64 {
    self.activations_created
  }

  /// Activation passivated count.
  #[must_use]
  pub const fn activations_passivated(&self) -> u64 {
    self.activations_passivated
  }
}
