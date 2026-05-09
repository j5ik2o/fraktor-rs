//! Typed restart supervision facade.

#[cfg(test)]
mod tests;

use core::time::Duration;

use fraktor_actor_core_rs::{
  actor::{
    error::ActorError,
    supervision::{
      RestartLimit, SupervisorDirective, SupervisorStrategy as KernelSupervisorStrategy, SupervisorStrategyConfig,
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
  /// Matches typed Pekko `Restart(maxRestarts = -1, withinTimeRange = Duration.Zero)`
  /// (see `references/pekko/actor-typed/.../SupervisorStrategy.scala:44-45`).
  /// `within = Duration::ZERO` is the fraktor-rs sentinel for "no window".
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self {
      inner: KernelSupervisorStrategy::new(
        SupervisorStrategyKind::OneForOne,
        RestartLimit::Unlimited,
        Duration::ZERO,
        restart_decider,
      )
      .with_stash_capacity(usize::MAX),
    }
  }

  /// Sets a finite restart limit (Pekko `maxNrOfRetries = max_restarts` with
  /// `max_restarts >= 0`). `max_restarts = 0` is a valid Pekko configuration
  /// meaning "no retry — stop immediately on the first failure" and is
  /// accepted without panic. Use [`Self::with_unlimited_restarts`] for
  /// unlimited retries.
  ///
  /// `within = Duration::ZERO` disables the window (matches typed Pekko
  /// `Duration.Zero` / classic Pekko `withinTimeRangeOption` returning
  /// `None`).
  #[must_use]
  pub fn with_limit(self, max_restarts: u32, within: Duration) -> Self {
    Self {
      inner: KernelSupervisorStrategy::new(
        self.kind(),
        RestartLimit::WithinWindow(max_restarts),
        within,
        restart_decider,
      )
      .with_stop_children(self.stop_children())
      .with_stash_capacity(self.stash_capacity())
      .with_logging_enabled(self.logging_enabled())
      .with_log_level(self.log_level()),
    }
  }

  /// Sets unlimited restarts within the given `within` window (Pekko
  /// `maxNrOfRetries = -1`). `within = Duration::ZERO` disables the window.
  #[must_use]
  pub fn with_unlimited_restarts(self, within: Duration) -> Self {
    Self {
      inner: KernelSupervisorStrategy::new(self.kind(), RestartLimit::Unlimited, within, restart_decider)
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

  /// Returns the configured restart limit policy.
  #[must_use]
  pub const fn max_restarts(&self) -> RestartLimit {
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
