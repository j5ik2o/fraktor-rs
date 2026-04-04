//! Typed restart supervision facade.

#[cfg(test)]
mod tests;

use core::time::Duration;

use crate::core::kernel::{
  actor::{
    error::ActorError,
    supervision::{
      SupervisorDirective, SupervisorStrategy as KernelSupervisorStrategy, SupervisorStrategyConfig,
      SupervisorStrategyKind,
    },
  },
  event::logging::LogLevel,
};

/// Pekko-compatible typed restart supervisor strategy facade.
#[derive(Clone, Debug)]
pub struct RestartSupervisorStrategy {
  inner: KernelSupervisorStrategy,
}

impl RestartSupervisorStrategy {
  /// Creates the default Pekko-compatible restart strategy with unlimited restarts.
  ///
  /// This default uses `max_restarts = 0` and `within = Duration::ZERO`, which
  /// matches Pekko's unlimited restart contract.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self {
      inner: KernelSupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 0, Duration::ZERO, restart_decider)
        .with_stash_capacity(usize::MAX),
    }
  }

  /// Sets the restart limit and rolling time window.
  ///
  /// `max_restarts = -1` mirrors Pekko's unlimited restart contract.
  /// `max_restarts = 0` is rejected to avoid silently selecting the same
  /// unlimited restart behavior; use `-1` for unlimited restarts.
  ///
  /// # Panics
  ///
  /// Panics when `max_restarts` is `0` or less than `-1`.
  #[must_use]
  pub fn with_limit(self, max_restarts: i32, within: Duration) -> Self {
    let max_restarts = if max_restarts == -1 {
      0
    } else {
      assert!(max_restarts != 0, "max_restarts must be -1 or at least 1");
      match u32::try_from(max_restarts) {
        | Ok(max_restarts) => max_restarts,
        | Err(_) => panic!("max_restarts must be -1 or at least 1"),
      }
    };
    Self {
      inner: KernelSupervisorStrategy::new(self.kind(), max_restarts, within, restart_decider)
        .with_stop_children(self.stop_children())
        .with_stash_capacity(self.stash_capacity())
        .with_logging_enabled(self.logging_enabled())
        .with_log_level(self.log_level()),
    }
  }

  /// Sets whether sibling children are stopped during restart.
  #[must_use]
  pub fn with_stop_children(mut self, stop_children: bool) -> Self {
    self.inner = self.inner.with_stop_children(stop_children);
    self
  }

  /// Sets the stash capacity used during restart.
  #[must_use]
  pub fn with_stash_capacity(mut self, stash_capacity: usize) -> Self {
    self.inner = self.inner.with_stash_capacity(stash_capacity);
    self
  }

  /// Enables or disables failure logging.
  #[must_use]
  pub fn with_logging_enabled(mut self, enabled: bool) -> Self {
    self.inner = self.inner.with_logging_enabled(enabled);
    self
  }

  /// Sets the failure log level.
  #[must_use]
  pub fn with_log_level(mut self, level: LogLevel) -> Self {
    self.inner = self.inner.with_log_level(level);
    self
  }

  /// Returns the supervision kind.
  #[must_use]
  pub const fn kind(&self) -> SupervisorStrategyKind {
    self.inner.kind()
  }

  /// Returns the configured restart limit.
  #[must_use]
  pub const fn max_restarts(&self) -> u32 {
    self.inner.max_restarts()
  }

  /// Returns the rolling restart window.
  #[must_use]
  pub const fn within(&self) -> Duration {
    self.inner.within()
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
}

impl From<RestartSupervisorStrategy> for KernelSupervisorStrategy {
  fn from(strategy: RestartSupervisorStrategy) -> Self {
    strategy.inner
  }
}

impl From<RestartSupervisorStrategy> for SupervisorStrategyConfig {
  fn from(strategy: RestartSupervisorStrategy) -> Self {
    Self::Standard(strategy.into())
  }
}

const fn restart_decider(_: &ActorError) -> SupervisorDirective {
  SupervisorDirective::Restart
}
