//! Typed backoff supervision facade.

#[cfg(test)]
mod tests;

use core::time::Duration;

use crate::core::kernel::{
  actor::supervision::{BackoffSupervisorStrategy as KernelBackoffSupervisorStrategy, SupervisorStrategyConfig},
  event::logging::LogLevel,
};

/// Pekko-compatible typed backoff supervisor strategy facade.
#[derive(Clone, Debug)]
pub struct BackoffSupervisorStrategy {
  inner: KernelBackoffSupervisorStrategy,
}

impl BackoffSupervisorStrategy {
  /// Creates the default typed backoff strategy.
  #[must_use]
  pub(crate) fn new(min_backoff: Duration, max_backoff: Duration, random_factor: f64) -> Self {
    Self {
      inner: KernelBackoffSupervisorStrategy::new(min_backoff, max_backoff, random_factor)
        .with_stash_capacity(usize::MAX)
        .with_critical_log_level(LogLevel::Error, u32::MAX),
    }
  }

  /// Sets the duration after which the backoff is reset.
  #[must_use]
  pub const fn with_reset_backoff_after(mut self, reset_backoff_after: Duration) -> Self {
    self.inner = self.inner.with_reset_backoff_after(reset_backoff_after);
    self
  }

  /// Sets the maximum number of restarts before giving up. 0 means unlimited.
  #[must_use]
  pub const fn with_max_restarts(mut self, max_restarts: u32) -> Self {
    self.inner = self.inner.with_max_restarts(max_restarts);
    self
  }

  /// Sets whether sibling children are stopped during restart.
  #[must_use]
  pub const fn with_stop_children(mut self, stop_children: bool) -> Self {
    self.inner = self.inner.with_stop_children(stop_children);
    self
  }

  /// Sets the stash capacity used during restart.
  #[must_use]
  pub const fn with_stash_capacity(mut self, stash_capacity: usize) -> Self {
    self.inner = self.inner.with_stash_capacity(stash_capacity);
    self
  }

  /// Enables or disables failure logging.
  #[must_use]
  pub const fn with_logging_enabled(mut self, enabled: bool) -> Self {
    self.inner = self.inner.with_logging_enabled(enabled);
    self
  }

  /// Sets the failure log level.
  #[must_use]
  pub const fn with_log_level(mut self, level: LogLevel) -> Self {
    self.inner = self.inner.with_log_level(level);
    self
  }

  /// Sets the critical log level and threshold after which it is used.
  #[must_use]
  pub const fn with_critical_log_level(mut self, level: LogLevel, after_errors: u32) -> Self {
    self.inner = self.inner.with_critical_log_level(level, after_errors);
    self
  }

  /// Returns the minimum backoff duration.
  #[must_use]
  pub const fn min_backoff(&self) -> Duration {
    self.inner.min_backoff()
  }

  /// Returns the maximum backoff duration.
  #[must_use]
  pub const fn max_backoff(&self) -> Duration {
    self.inner.max_backoff()
  }

  /// Returns the random jitter factor.
  #[must_use]
  pub const fn random_factor(&self) -> f64 {
    self.inner.random_factor()
  }

  /// Returns the duration after which the backoff is reset.
  #[must_use]
  pub const fn reset_backoff_after(&self) -> Duration {
    self.inner.reset_backoff_after()
  }

  /// Returns the maximum number of restarts. 0 means unlimited.
  #[must_use]
  pub const fn max_restarts(&self) -> u32 {
    self.inner.max_restarts()
  }

  /// Returns whether sibling children are stopped during restart.
  #[must_use]
  pub const fn stop_children(&self) -> bool {
    self.inner.stop_children()
  }

  /// Returns the stash capacity.
  #[must_use]
  pub const fn stash_capacity(&self) -> usize {
    self.inner.stash_capacity()
  }

  /// Returns whether failure logging is enabled.
  #[must_use]
  pub const fn logging_enabled(&self) -> bool {
    self.inner.logging_enabled()
  }

  /// Returns the configured failure log level.
  #[must_use]
  pub const fn log_level(&self) -> LogLevel {
    self.inner.log_level()
  }

  /// Returns the critical log level applied after the configured threshold.
  #[must_use]
  pub const fn critical_log_level(&self) -> LogLevel {
    self.inner.critical_log_level()
  }

  /// Returns the error count threshold after which the critical log level is used.
  #[must_use]
  pub const fn critical_log_level_after(&self) -> u32 {
    self.inner.critical_log_level_after()
  }
}

impl From<BackoffSupervisorStrategy> for KernelBackoffSupervisorStrategy {
  fn from(strategy: BackoffSupervisorStrategy) -> Self {
    strategy.inner
  }
}

impl From<BackoffSupervisorStrategy> for SupervisorStrategyConfig {
  fn from(strategy: BackoffSupervisorStrategy) -> Self {
    Self::Backoff(strategy.into())
  }
}
