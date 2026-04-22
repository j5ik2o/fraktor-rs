//! Backoff-based supervisor strategy with exponential delay calculation.

use core::time::Duration;

use super::restart_limit::RestartLimit;
use crate::core::kernel::event::logging::LogLevel;

#[cfg(test)]
mod tests;

const DEFAULT_STASH_CAPACITY: usize = 1000;

/// Supervisor strategy providing exponential backoff for restart delays.
///
/// The backoff delay is computed as `min(max_backoff, min_backoff * 2^restart_iteration)`.
/// An optional jitter can be applied via
/// [`compute_backoff_with_jitter`](Self::compute_backoff_with_jitter).
///
/// Pekko parity note: Pekko's `BackoffOnRestartSupervisor` installs an internal
/// `OneForOneStrategy` configured with the user-provided
/// `OneForOneStrategy(maxNrOfRetries, withinTimeRange, ...)` (see
/// `references/pekko/actor/src/main/scala/org/apache/pekko/pattern/internal/
/// BackoffOnRestartSupervisor.scala:58`). `withinTimeRange` there is the retry-accounting window
/// (the classic `maxNrOfRetries.withinTimeRange` concept) and is **distinct from**
/// `resetBackoffAfter`, which controls when the backoff iteration counter
/// resets after the child stabilizes. fraktor-rs mirrors this separation
/// with two independent fields: [`within_time_range`](Self::within_time_range)
/// for retry accounting and [`reset_backoff_after`](Self::reset_backoff_after)
/// for backoff reset.
#[derive(Clone, Debug)]
pub struct BackoffSupervisorStrategy {
  min_backoff:              Duration,
  max_backoff:              Duration,
  random_factor:            f64,
  reset_backoff_after:      Duration,
  within_time_range:        Duration,
  max_restarts:             RestartLimit,
  stop_children:            bool,
  stash_capacity:           usize,
  logging_enabled:          bool,
  log_level:                LogLevel,
  critical_log_level:       LogLevel,
  critical_log_level_after: u32,
}

impl BackoffSupervisorStrategy {
  /// Creates a new backoff supervisor strategy.
  ///
  /// `reset_backoff_after` defaults to `(min_backoff + max_backoff) / 2`.
  ///
  /// # Panics
  ///
  /// Panics if `min_backoff > max_backoff` or `random_factor`
  /// is outside `[0.0, 1.0]`.
  #[must_use]
  pub fn new(min_backoff: Duration, max_backoff: Duration, random_factor: f64) -> Self {
    assert!(min_backoff <= max_backoff, "min_backoff must be <= max_backoff");
    assert!((0.0..=1.0).contains(&random_factor) && !random_factor.is_nan(), "random_factor must be in [0.0, 1.0]");
    let reset_backoff_after = (min_backoff + max_backoff) / 2;
    Self {
      min_backoff,
      max_backoff,
      random_factor,
      reset_backoff_after,
      // Default `Duration::ZERO` = no window (matches typed Pekko
      // `Duration.Zero` / classic Pekko `withinTimeRangeOption == None`).
      // Combined with `max_restarts = Unlimited` this yields truly unlimited
      // restarts by default, matching Pekko's `OneForOneStrategy()` default.
      within_time_range: Duration::ZERO,
      max_restarts: RestartLimit::Unlimited,
      stop_children: true,
      stash_capacity: DEFAULT_STASH_CAPACITY,
      logging_enabled: true,
      log_level: LogLevel::Error,
      critical_log_level: LogLevel::Error,
      critical_log_level_after: 0,
    }
  }

  /// Computes the deterministic backoff delay for the given restart iteration.
  ///
  /// `restart_iteration = 0` represents the first delayed restart attempt and
  /// therefore returns `min_backoff`.
  ///
  /// Formula: `min(max_backoff, min_backoff * 2^restart_iteration)`.
  /// Overflow-safe: caps at `max_backoff` when the multiplication would overflow.
  #[must_use]
  pub fn compute_backoff(&self, restart_iteration: u32) -> Duration {
    let base = self.min_backoff.as_nanos();
    let factor = 1u128.checked_shl(restart_iteration).unwrap_or(u128::MAX);
    let delay_nanos = base.saturating_mul(factor);
    let max_nanos = self.max_backoff.as_nanos();
    let capped = delay_nanos.min(max_nanos);
    safe_nanos_to_duration(capped)
  }

  /// Computes the backoff delay with jitter applied.
  ///
  /// `restart_iteration = 0` represents the first delayed restart attempt.
  ///
  /// Formula: `compute_backoff(restart_iteration) * (1.0 + random * random_factor)`.
  /// The `random` parameter should be in `[0.0, 1.0]`.
  #[must_use]
  pub fn compute_backoff_with_jitter(&self, restart_iteration: u32, random: f64) -> Duration {
    let base = self.compute_backoff(restart_iteration);
    let random = if random.is_nan() { 0.0 } else { random.clamp(0.0, 1.0) };
    let jitter_multiplier = 1.0 + random * self.random_factor;
    let nanos = (base.as_nanos() as f64 * jitter_multiplier) as u128;
    let max_nanos = self.max_backoff.as_nanos();
    let capped = nanos.min(max_nanos);
    safe_nanos_to_duration(capped)
  }

  /// Sets the duration after which the backoff is reset.
  #[must_use]
  pub const fn with_reset_backoff_after(mut self, reset_backoff_after: Duration) -> Self {
    self.reset_backoff_after = reset_backoff_after;
    self
  }

  /// Sets the maximum number of restarts using the [`RestartLimit`] contract
  /// (Pekko `maxNrOfRetries`). Use [`RestartLimit::Unlimited`] for no bound,
  /// [`RestartLimit::WithinWindow`]`(0)` for immediate stop, and
  /// [`RestartLimit::WithinWindow`]`(n)` for up to `n` retries.
  #[must_use]
  pub const fn with_max_restarts(mut self, max_restarts: RestartLimit) -> Self {
    self.max_restarts = max_restarts;
    self
  }

  /// Sets the retry-accounting window (Pekko `OneForOneStrategy.withinTimeRange`).
  ///
  /// Pass [`Duration::ZERO`] to disable the window (matches typed Pekko
  /// `Duration.Zero` and classic Pekko `withinTimeRangeOption == None`).
  /// This is independent of [`reset_backoff_after`](Self::reset_backoff_after),
  /// which controls the backoff-iteration reset timer.
  #[must_use]
  pub const fn with_within_time_range(mut self, within: Duration) -> Self {
    self.within_time_range = within;
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

  /// Returns the configured restart limit policy.
  #[must_use]
  pub const fn max_restarts(&self) -> RestartLimit {
    self.max_restarts
  }

  /// Returns the retry-accounting window (Pekko `withinTimeRange`).
  ///
  /// `Duration::ZERO` means "no window" (disabled).
  #[must_use]
  pub const fn within_time_range(&self) -> Duration {
    self.within_time_range
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

  /// Returns whether failure logging is enabled.
  #[must_use]
  pub const fn logging_enabled(&self) -> bool {
    self.logging_enabled
  }

  /// Returns the log level used for failure events.
  #[must_use]
  pub const fn log_level(&self) -> LogLevel {
    self.log_level
  }

  /// Returns the critical log level applied after a threshold of errors.
  #[must_use]
  pub const fn critical_log_level(&self) -> LogLevel {
    self.critical_log_level
  }

  /// Returns the error count threshold after which the critical log level is used.
  #[must_use]
  pub const fn critical_log_level_after(&self) -> u32 {
    self.critical_log_level_after
  }

  /// Returns the effective log level for the given error count.
  ///
  /// When `critical_log_level_after` is non-zero and `error_count` exceeds that threshold,
  /// the critical log level is returned instead of the normal log level.
  #[must_use]
  pub const fn effective_log_level(&self, error_count: u32) -> LogLevel {
    if self.critical_log_level_after > 0 && error_count >= self.critical_log_level_after {
      self.critical_log_level
    } else {
      self.log_level
    }
  }

  /// Sets whether failure logging is enabled.
  #[must_use]
  pub const fn with_logging_enabled(mut self, enabled: bool) -> Self {
    self.logging_enabled = enabled;
    self
  }

  /// Sets the log level for failure events.
  #[must_use]
  pub const fn with_log_level(mut self, level: LogLevel) -> Self {
    self.log_level = level;
    self
  }

  /// Sets the critical log level and the error count threshold after which it is applied.
  ///
  /// When the number of consecutive errors reaches `after_errors`, subsequent failures
  /// are logged at `level` instead of the normal log level.
  #[must_use]
  pub const fn with_critical_log_level(mut self, level: LogLevel, after_errors: u32) -> Self {
    self.critical_log_level = level;
    self.critical_log_level_after = after_errors;
    self
  }
}

/// Converts a u128 nanosecond value to [`Duration`], clamping at `u64::MAX` to avoid truncation.
fn safe_nanos_to_duration(nanos: u128) -> Duration {
  let clamped = if nanos > u128::from(u64::MAX) { u64::MAX } else { nanos as u64 };
  Duration::from_nanos(clamped)
}
