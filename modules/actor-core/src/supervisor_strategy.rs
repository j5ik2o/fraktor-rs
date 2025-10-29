//! Supervisor strategy definitions.

use core::{fmt, time::Duration};

use crate::actor_error::ActorError;

/// Decision emitted by a supervisor strategy when handling an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorDirective {
  /// Resume processing without restarting the actor.
  Resume,
  /// Restart the failing actor.
  Restart,
  /// Stop the failing actor.
  Stop,
  /// Escalate to the parent supervisor.
  Escalate,
}

/// Supervisor fan-out strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyKind {
  /// Only the failing child is affected.
  OneForOne,
  /// The entire sibling group responds together.
  AllForOne,
}

type DeciderFn<'a> = dyn Fn(&ActorError) -> SupervisorDirective + 'a;

/// Configurable supervisor strategy with restart limits.
#[derive(Clone, Copy)]
pub struct SupervisorStrategy<'a> {
  kind: StrategyKind,
  max_restarts: u32,
  within: Duration,
  decider: Option<&'a DeciderFn<'a>>,
}

impl<'a> fmt::Debug for SupervisorStrategy<'a> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f
      .debug_struct("SupervisorStrategy")
      .field("kind", &self.kind)
      .field("max_restarts", &self.max_restarts)
      .field("within", &self.within)
      .field("has_decider", &self.decider.is_some())
      .finish()
  }
}

impl<'a> SupervisorStrategy<'a> {
  /// Creates a new strategy with the provided kind and restart budget.
  #[must_use]
  pub const fn new(kind: StrategyKind, max_restarts: u32, within: Duration) -> Self {
    Self { kind, max_restarts, within, decider: None }
  }

  /// Specifies a custom decider.
  pub fn with_decider(mut self, decider: &'a DeciderFn<'a>) -> Self {
    self.decider = Some(decider);
    self
  }

  /// Returns the configured strategy kind.
  #[must_use]
  pub const fn kind(&self) -> StrategyKind {
    self.kind
  }

  /// Returns the maximum restart count within the time window.
  #[must_use]
  pub const fn max_restarts(&self) -> u32 {
    self.max_restarts
  }

  /// Returns the restart counter reset window.
  #[must_use]
  pub const fn reset_interval(&self) -> Duration {
    self.within
  }

  /// Computes the directive for the provided error.
  #[must_use]
  pub fn decide(&self, error: &ActorError) -> SupervisorDirective {
    if let Some(decider) = self.decider {
      return decider(error);
    }
    match error {
      ActorError::Recoverable(_) => SupervisorDirective::Restart,
      ActorError::Fatal(_) => SupervisorDirective::Stop,
    }
  }
}
