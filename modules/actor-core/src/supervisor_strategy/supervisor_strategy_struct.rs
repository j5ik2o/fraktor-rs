use core::time::Duration;

use super::{supervisor_directive::SupervisorDirective, supervisor_strategy_kind::SupervisorStrategyKind};
use crate::actor_error::ActorError;

type SupervisorDecider = fn(&ActorError) -> SupervisorDirective;

/// Supervisor configuration controlling restart policies.
#[derive(Clone, Copy, Debug)]
pub struct SupervisorStrategy {
  kind:         SupervisorStrategyKind,
  max_restarts: u32,
  within:       Duration,
  decider:      SupervisorDecider,
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
    Self { kind, max_restarts, within, decider }
  }

  /// Evaluates the supervisor directive for the provided error.
  #[must_use]
  pub fn decide(&self, error: &ActorError) -> SupervisorDirective {
    (self.decider)(error)
  }

  /// Returns the strategy kind.
  #[must_use]
  pub const fn kind(&self) -> SupervisorStrategyKind {
    self.kind
  }

  /// Returns the restart limit.
  #[must_use]
  pub const fn max_restarts(&self) -> u32 {
    self.max_restarts
  }

  /// Returns the time window for restart counting.
  #[must_use]
  pub const fn within(&self) -> Duration {
    self.within
  }
}
