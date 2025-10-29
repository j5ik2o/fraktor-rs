//! Supervisor strategy definitions controlling restart semantics.

mod decision;
mod kind;

use core::time::Duration;

use cellactor_utils_core_rs::ArcShared;
pub use decision::SupervisorDecision;
pub use kind::StrategyKind;

use crate::actor_error::ActorError;

type SupervisorDecider = ArcShared<dyn Fn(&ActorError) -> SupervisorDecision + Send + Sync + 'static>;

/// Configuration describing how a supervisor reacts to child failures.
#[derive(Clone)]
pub struct SupervisorStrategy {
  kind:           StrategyKind,
  max_restarts:   u32,
  reset_interval: Option<Duration>,
  decider:        Option<SupervisorDecider>,
}

impl SupervisorStrategy {
  /// Creates a one-for-one strategy using default thresholds.
  #[must_use]
  pub fn one_for_one() -> Self {
    Self { kind: StrategyKind::OneForOne, max_restarts: 10, reset_interval: None, decider: None }
  }

  /// Creates an all-for-one strategy using default thresholds.
  #[must_use]
  pub fn all_for_one() -> Self {
    Self { kind: StrategyKind::AllForOne, max_restarts: 10, reset_interval: None, decider: None }
  }

  /// Overrides the restart limit applied within the reset interval.
  #[must_use]
  pub fn with_max_restarts(mut self, max: u32) -> Self {
    self.max_restarts = max;
    self
  }

  /// Sets the observation window for counting restarts.
  #[must_use]
  pub fn with_reset_interval(mut self, interval: Duration) -> Self {
    self.reset_interval = Some(interval);
    self
  }

  /// Supplies a custom decider invoked on failures.
  #[must_use]
  pub fn with_decider(mut self, decider: SupervisorDecider) -> Self {
    self.decider = Some(decider);
    self
  }

  /// Returns the strategy kind.
  #[must_use]
  pub const fn kind(&self) -> StrategyKind {
    self.kind
  }

  /// Returns the maximum number of restarts allowed.
  #[must_use]
  pub const fn max_restarts(&self) -> u32 {
    self.max_restarts
  }

  /// Returns the reset interval when configured.
  #[must_use]
  pub const fn reset_interval(&self) -> Option<Duration> {
    self.reset_interval
  }

  /// Evaluates the strategy's decider against the provided error.
  #[must_use]
  pub fn decide(&self, error: &ActorError) -> SupervisorDecision {
    if let Some(decider) = &self.decider {
      return decider(error);
    }

    if error.is_recoverable() { SupervisorDecision::Restart } else { SupervisorDecision::Stop }
  }
}
