use core::time::Duration;

use crate::actor_error::ActorError;

/// Directive returned by supervisor strategies when handling failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorDirective {
  /// Restart the failing actor.
  Restart,
  /// Stop the failing actor permanently.
  Stop,
  /// Escalate the failure to the parent supervisor.
  Escalate,
}

/// Supervisor strategy variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorStrategyKind {
  /// Only the failing actor is affected.
  OneForOne,
  /// Sibling actors are also restarted when a failure occurs.
  AllForOne,
}

/// Supervisor configuration controlling restart policies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupervisorStrategy {
  kind:         SupervisorStrategyKind,
  max_restarts: u32,
  within:       Duration,
  decider:      SupervisorDecider,
}

type SupervisorDecider = fn(&ActorError) -> SupervisorDirective;

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
