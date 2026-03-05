//! Backoff-based supervisor strategy with exponential delay calculation.

use core::time::Duration;

#[cfg(test)]
mod tests;

/// Supervisor strategy providing exponential backoff for restart delays.
///
/// The backoff delay is computed as `min(max_backoff, min_backoff * 2^restart_count)`.
/// An optional jitter can be applied via
/// [`compute_backoff_with_jitter`](Self::compute_backoff_with_jitter).
#[derive(Clone, Debug)]
pub struct BackoffSupervisorStrategy {
  min_backoff:         Duration,
  max_backoff:         Duration,
  random_factor:       f64,
  reset_backoff_after: Duration,
  max_restarts:        u32,
  stop_children:       bool,
  stash_capacity:      usize,
}

impl BackoffSupervisorStrategy {
  /// Creates a new backoff supervisor strategy.
  ///
  /// `reset_backoff_after` defaults to `(min_backoff + max_backoff) / 2`.
  #[must_use]
  pub fn new(min_backoff: Duration, max_backoff: Duration, random_factor: f64) -> Self {
    let reset_backoff_after = (min_backoff + max_backoff) / 2;
    Self {
      min_backoff,
      max_backoff,
      random_factor,
      reset_backoff_after,
      max_restarts: 0,
      stop_children: true,
      stash_capacity: 1000,
    }
  }

  /// Computes the deterministic backoff delay for the given restart count.
  ///
  /// Formula: `min(max_backoff, min_backoff * 2^restart_count)`.
  /// Overflow-safe: caps at `max_backoff` when the multiplication would overflow.
  #[must_use]
  pub fn compute_backoff(&self, restart_count: u32) -> Duration {
    let base = self.min_backoff.as_nanos();
    let factor = 1u128.checked_shl(restart_count).unwrap_or(u128::MAX);
    let delay_nanos = base.saturating_mul(factor);
    let max_nanos = self.max_backoff.as_nanos();
    let capped = delay_nanos.min(max_nanos);
    Duration::from_nanos(capped as u64)
  }

  /// Computes the backoff delay with jitter applied.
  ///
  /// Formula: `compute_backoff(restart_count) * (1.0 + random * random_factor)`.
  /// The `random` parameter should be in `[0.0, 1.0]`.
  #[must_use]
  pub fn compute_backoff_with_jitter(&self, restart_count: u32, random: f64) -> Duration {
    let base = self.compute_backoff(restart_count);
    let jitter_multiplier = 1.0 + random * self.random_factor;
    let nanos = (base.as_nanos() as f64 * jitter_multiplier) as u128;
    let max_nanos = self.max_backoff.as_nanos();
    let capped = nanos.min(max_nanos);
    Duration::from_nanos(capped as u64)
  }

  /// Sets the duration after which the backoff is reset.
  #[must_use]
  pub const fn with_reset_backoff_after(mut self, reset_backoff_after: Duration) -> Self {
    self.reset_backoff_after = reset_backoff_after;
    self
  }

  /// Sets the maximum number of restarts before giving up. 0 means unlimited.
  #[must_use]
  pub const fn with_max_restarts(mut self, max_restarts: u32) -> Self {
    self.max_restarts = max_restarts;
    self
  }

  /// Sets whether sibling children should be stopped on restart.
  #[must_use]
  pub const fn with_stop_children(mut self, stop_children: bool) -> Self {
    self.stop_children = stop_children;
    self
  }

  /// Sets the stash capacity for message buffering during restart.
  #[must_use]
  pub const fn with_stash_capacity(mut self, stash_capacity: usize) -> Self {
    self.stash_capacity = stash_capacity;
    self
  }

  /// Returns the minimum backoff duration.
  #[must_use]
  pub const fn min_backoff(&self) -> Duration {
    self.min_backoff
  }

  /// Returns the maximum backoff duration.
  #[must_use]
  pub const fn max_backoff(&self) -> Duration {
    self.max_backoff
  }

  /// Returns the random jitter factor.
  #[must_use]
  pub const fn random_factor(&self) -> f64 {
    self.random_factor
  }

  /// Returns the duration after which the backoff is reset.
  #[must_use]
  pub const fn reset_backoff_after(&self) -> Duration {
    self.reset_backoff_after
  }

  /// Returns the maximum number of restarts. 0 means unlimited.
  #[must_use]
  pub const fn max_restarts(&self) -> u32 {
    self.max_restarts
  }

  /// Returns whether sibling children are stopped on restart.
  #[must_use]
  pub const fn stop_children(&self) -> bool {
    self.stop_children
  }

  /// Returns the stash capacity.
  #[must_use]
  pub const fn stash_capacity(&self) -> usize {
    self.stash_capacity
  }
}
