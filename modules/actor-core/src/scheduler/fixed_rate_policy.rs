//! Policy parameters applied to fixed-rate scheduler jobs.

use core::num::NonZeroU32;

/// Controls backlog and warning behavior for fixed-rate jobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedRatePolicy {
  backlog_limit:  NonZeroU32,
  burst_threshold: NonZeroU32,
}

impl FixedRatePolicy {
  /// Creates a new policy instance.
  #[must_use]
  pub const fn new(backlog_limit: NonZeroU32, burst_threshold: NonZeroU32) -> Self {
    Self { backlog_limit, burst_threshold }
  }

  /// Returns the configured backlog limit.
  #[must_use]
  pub const fn backlog_limit(&self) -> NonZeroU32 {
    self.backlog_limit
  }

  /// Returns the configured burst warning threshold.
  #[must_use]
  pub const fn burst_threshold(&self) -> NonZeroU32 {
    self.burst_threshold
  }

  /// Overrides the backlog limit, returning a new policy.
  #[must_use]
  pub const fn with_backlog_limit(mut self, backlog_limit: NonZeroU32) -> Self {
    self.backlog_limit = backlog_limit;
    self
  }

  /// Overrides the burst threshold, returning a new policy.
  #[must_use]
  pub const fn with_burst_threshold(mut self, burst_threshold: NonZeroU32) -> Self {
    self.burst_threshold = burst_threshold;
    self
  }
}

impl Default for FixedRatePolicy {
  fn default() -> Self {
    let backlog_limit = NonZeroU32::new(4).expect("non-zero backlog limit");
    let burst_threshold = NonZeroU32::new(8).expect("non-zero burst threshold");
    Self::new(backlog_limit, burst_threshold)
  }
}
