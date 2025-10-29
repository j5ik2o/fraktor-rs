//! Supervisor options stored within Props.

use core::time::Duration;

use crate::supervisor_strategy::{StrategyKind, SupervisorDirective, SupervisorStrategy};

/// Supervisor configuration applied to spawned actors.
#[derive(Debug, Clone, Copy)]
pub struct SupervisorOptions {
  strategy:             SupervisorStrategy<'static>,
  escalation_threshold: u32,
}

impl SupervisorOptions {
  /// Creates options from the provided strategy.
  #[must_use]
  pub const fn new(strategy: SupervisorStrategy<'static>, escalation_threshold: u32) -> Self {
    Self { strategy, escalation_threshold }
  }

  /// Returns the configured supervisor strategy.
  #[must_use]
  pub const fn strategy(&self) -> SupervisorStrategy<'static> {
    self.strategy
  }

  /// Returns the escalation threshold (number of consecutive fatals allowed).
  #[must_use]
  pub const fn escalation_threshold(&self) -> u32 {
    self.escalation_threshold
  }

  /// Evaluates the strategy for the provided error.
  #[must_use]
  pub fn decide(&self, error: &crate::actor_error::ActorError) -> SupervisorDirective {
    self.strategy.decide(error)
  }
}

impl Default for SupervisorOptions {
  fn default() -> Self {
    let strategy = SupervisorStrategy::new(StrategyKind::OneForOne, 10, Duration::from_secs(1));
    Self::new(strategy, 3)
  }
}
