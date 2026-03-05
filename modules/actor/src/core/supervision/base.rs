//! Supervisor configuration and decision logic.

use core::time::Duration;

use super::{supervisor_directive::SupervisorDirective, supervisor_strategy_kind::SupervisorStrategyKind};
use crate::core::{error::ActorError, supervision::restart_statistics::RestartStatistics};

#[cfg(test)]
mod tests;

type SupervisorDecider = fn(&ActorError) -> SupervisorDirective;

/// Supervisor configuration controlling restart policies.
#[derive(Clone, Debug)]
pub struct SupervisorStrategy {
  kind:           SupervisorStrategyKind,
  max_restarts:   u32,
  within:         Duration,
  decider:        SupervisorDecider,
  stop_children:  bool,
  stash_capacity: usize,
}

impl SupervisorStrategy {
  /// Creates a supervisor strategy.
  #[must_use]
  pub const fn new(
    kind: SupervisorStrategyKind,
    max_restarts: u32,
    within: Duration,
    decider: SupervisorDecider,
  ) -> Self {
    Self { kind, max_restarts, within, decider, stop_children: true, stash_capacity: 1000 }
  }

  /// Evaluates the supervisor directive for the provided error.
  #[must_use]
  pub fn decide(&self, error: &ActorError) -> SupervisorDirective {
    (self.decider)(error)
  }

  /// Applies restart accounting and returns the effective directive.
  ///
  /// When the decider returns [`SupervisorDirective::Restart`], the failure count is tracked within
  /// the configured `within` window. If the restart count exceeds `max_restarts`, the directive is
  /// promoted to [`SupervisorDirective::Stop`]. Any other directive resets the statistics.
  #[must_use]
  pub fn handle_failure(
    &self,
    statistics: &mut RestartStatistics,
    error: &ActorError,
    now: Duration,
  ) -> SupervisorDirective {
    match self.decide(error) {
      | SupervisorDirective::Restart => {
        let limit = if self.max_restarts == 0 { None } else { Some(self.max_restarts) };
        let count = statistics.record_failure(now, self.within, limit);
        if self.max_restarts > 0 && count as u32 > self.max_restarts {
          statistics.reset();
          SupervisorDirective::Stop
        } else {
          SupervisorDirective::Restart
        }
      },
      | SupervisorDirective::Stop => {
        statistics.reset();
        SupervisorDirective::Stop
      },
      | SupervisorDirective::Escalate => {
        statistics.reset();
        SupervisorDirective::Escalate
      },
      | SupervisorDirective::Resume => SupervisorDirective::Resume,
    }
  }

  /// Returns the strategy kind.
  #[must_use]
  pub const fn kind(&self) -> SupervisorStrategyKind {
    self.kind
  }

  /// Returns the restart threshold.
  #[must_use]
  pub const fn max_restarts(&self) -> u32 {
    self.max_restarts
  }

  /// Returns the time window used when counting restarts.
  #[must_use]
  pub const fn within(&self) -> Duration {
    self.within
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
}

impl Default for SupervisorStrategy {
  fn default() -> Self {
    const fn decider(error: &ActorError) -> SupervisorDirective {
      match error {
        | ActorError::Recoverable(_) => SupervisorDirective::Restart,
        | ActorError::Fatal(_) => SupervisorDirective::Stop,
      }
    }

    Self::new(SupervisorStrategyKind::OneForOne, 10, Duration::from_secs(1), decider)
  }
}
