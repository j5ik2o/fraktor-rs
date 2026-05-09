//! Unified supervisor strategy selection.

#[cfg(test)]
mod tests;

use core::time::Duration;

use super::{
  backoff_supervisor_strategy::BackoffSupervisorStrategy, base::SupervisorStrategy,
  restart_statistics::RestartStatistics, supervisor_directive::SupervisorDirective,
  supervisor_strategy_kind::SupervisorStrategyKind,
};
use crate::{actor::error::ActorError, event::logging::LogLevel};

/// Configuration selecting either a standard or backoff supervisor strategy.
#[derive(Clone, Debug)]
pub enum SupervisorStrategyConfig {
  /// Standard restart strategy with fixed restart limits.
  Standard(SupervisorStrategy),
  /// Backoff strategy with exponential delay between restarts.
  Backoff(BackoffSupervisorStrategy),
}

impl SupervisorStrategyConfig {
  /// Evaluates the supervisor directive for the provided error.
  #[must_use]
  pub fn decide(&self, error: &ActorError) -> SupervisorDirective {
    match self {
      | Self::Standard(s) => s.decide(error),
      | Self::Backoff(_) => backoff_decide(error),
    }
  }

  /// Applies restart accounting and returns the effective directive.
  #[must_use]
  pub fn handle_failure(
    &self,
    statistics: &mut RestartStatistics,
    error: &ActorError,
    now: Duration,
  ) -> SupervisorDirective {
    match self {
      | Self::Standard(s) => s.handle_failure(statistics, error, now),
      | Self::Backoff(b) => Self::handle_backoff_failure(b, statistics, error, now),
    }
  }

  fn handle_backoff_failure(
    backoff: &BackoffSupervisorStrategy,
    statistics: &mut RestartStatistics,
    error: &ActorError,
    now: Duration,
  ) -> SupervisorDirective {
    let directive = backoff_decide(error);
    match directive {
      | SupervisorDirective::Restart => {
        // Pekko `BackoffOnRestartSupervisor` installs an internal
        // `OneForOneStrategy(maxNrOfRetries, withinTimeRange, ...)` using the
        // user-provided strategy's `withinTimeRange` — NOT `resetBackoffAfter`,
        // which controls backoff iteration reset and is a separate concept.
        // See references/pekko/actor/src/main/scala/.../BackoffOnRestartSupervisor.scala:58.
        if statistics.request_restart_permission(now, backoff.max_restarts(), backoff.within_time_range()) {
          SupervisorDirective::Restart
        } else {
          statistics.reset();
          SupervisorDirective::Stop
        }
      },
      | SupervisorDirective::Resume => SupervisorDirective::Resume,
      | other => {
        statistics.reset();
        other
      },
    }
  }

  /// Returns the strategy kind.
  ///
  /// Backoff strategies always use [`SupervisorStrategyKind::OneForOne`].
  #[must_use]
  pub const fn kind(&self) -> SupervisorStrategyKind {
    match self {
      | Self::Standard(s) => s.kind(),
      | Self::Backoff(_) => SupervisorStrategyKind::OneForOne,
    }
  }

  /// Returns whether sibling children are stopped on restart.
  #[must_use]
  pub const fn stop_children(&self) -> bool {
    match self {
      | Self::Standard(s) => s.stop_children(),
      | Self::Backoff(b) => b.stop_children(),
    }
  }

  /// Returns the stash capacity.
  #[must_use]
  pub const fn stash_capacity(&self) -> usize {
    match self {
      | Self::Standard(s) => s.stash_capacity(),
      | Self::Backoff(b) => b.stash_capacity(),
    }
  }

  /// Returns whether failure logging is enabled for this strategy.
  #[must_use]
  pub const fn logging_enabled(&self) -> bool {
    match self {
      | Self::Standard(s) => s.logging_enabled(),
      | Self::Backoff(b) => b.logging_enabled(),
    }
  }

  /// Returns the effective log level considering the error count.
  ///
  /// For standard strategies the configured log level is returned regardless of the count.
  /// For backoff strategies the critical log level is used once the count exceeds the
  /// configured threshold.
  #[must_use]
  pub const fn effective_log_level(&self, error_count: u32) -> LogLevel {
    match self {
      | Self::Standard(s) => s.log_level(),
      | Self::Backoff(b) => b.effective_log_level(error_count),
    }
  }
}

impl Default for SupervisorStrategyConfig {
  fn default() -> Self {
    Self::Standard(SupervisorStrategy::default())
  }
}

impl From<SupervisorStrategy> for SupervisorStrategyConfig {
  fn from(strategy: SupervisorStrategy) -> Self {
    Self::Standard(strategy)
  }
}

impl From<BackoffSupervisorStrategy> for SupervisorStrategyConfig {
  fn from(strategy: BackoffSupervisorStrategy) -> Self {
    Self::Backoff(strategy)
  }
}

/// Default backoff error→directive mapping: recoverable → Restart, fatal → Stop, escalate →
/// Escalate.
const fn backoff_decide(error: &ActorError) -> SupervisorDirective {
  match error {
    | ActorError::Recoverable(_) => SupervisorDirective::Restart,
    | ActorError::Fatal(_) => SupervisorDirective::Stop,
    | ActorError::Escalate(_) => SupervisorDirective::Escalate,
  }
}
