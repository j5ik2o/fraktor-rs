//! Typed supervisor strategy factories.

#[cfg(test)]
mod tests;

use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    error::ActorError,
    supervision::{
      RestartLimit, SupervisorDirective, SupervisorStrategy as KernelSupervisorStrategy, SupervisorStrategyConfig,
      SupervisorStrategyKind,
    },
  },
  event::logging::LogLevel,
};

use crate::{BackoffSupervisorStrategy, RestartSupervisorStrategy};

/// Pekko-compatible typed standard supervisor strategy facade.
#[derive(Clone, Debug)]
pub struct SupervisorStrategy {
  inner: KernelSupervisorStrategy,
}

impl SupervisorStrategy {
  /// Creates a strategy that resumes the actor on failure.
  ///
  /// `max_restarts` is set to [`RestartLimit::WithinWindow`]`(0)` since the
  /// decider never returns `Restart`; this matches Pekko's convention for
  /// strategies that don't use the retry budget.
  #[must_use]
  pub const fn resume() -> Self {
    Self {
      inner: KernelSupervisorStrategy::new(
        SupervisorStrategyKind::OneForOne,
        RestartLimit::WithinWindow(0),
        Duration::ZERO,
        resume_decider,
      )
      .with_stash_capacity(usize::MAX),
    }
  }

  /// Creates a strategy that restarts the actor on failure.
  #[must_use]
  pub const fn restart() -> RestartSupervisorStrategy {
    RestartSupervisorStrategy::new()
  }

  /// Creates a strategy that restarts the actor with exponential backoff.
  #[must_use]
  pub fn restart_with_backoff(
    min_backoff: Duration,
    max_backoff: Duration,
    random_factor: f64,
  ) -> BackoffSupervisorStrategy {
    BackoffSupervisorStrategy::new(min_backoff, max_backoff, random_factor)
  }

  /// Creates a strategy that stops the actor on failure.
  ///
  /// `max_restarts` is set to [`RestartLimit::WithinWindow`]`(0)` — matching
  /// Pekko `stoppingStrategy` which never restarts.
  #[must_use]
  pub const fn stop() -> Self {
    Self {
      inner: KernelSupervisorStrategy::new(
        SupervisorStrategyKind::OneForOne,
        RestartLimit::WithinWindow(0),
        Duration::ZERO,
        stop_decider,
      )
      .with_stash_capacity(usize::MAX),
    }
  }

  /// Returns the supervision kind.
  #[must_use]
  pub const fn kind(&self) -> SupervisorStrategyKind {
    self.inner.kind()
  }

  /// Evaluates the directive for the provided error.
  #[must_use]
  pub fn decide(&self, error: &ActorError) -> SupervisorDirective {
    self.inner.decide(error)
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
}

impl From<SupervisorStrategy> for KernelSupervisorStrategy {
  fn from(strategy: SupervisorStrategy) -> Self {
    strategy.inner
  }
}

impl From<SupervisorStrategy> for SupervisorStrategyConfig {
  fn from(strategy: SupervisorStrategy) -> Self {
    Self::Standard(strategy.into())
  }
}

const fn resume_decider(_: &ActorError) -> SupervisorDirective {
  SupervisorDirective::Resume
}

const fn stop_decider(_: &ActorError) -> SupervisorDirective {
  SupervisorDirective::Stop
}
