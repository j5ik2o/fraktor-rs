use crate::supervisor_strategy::SupervisorStrategy;

/// Supervisor configuration stored alongside [`Props`].
#[derive(Clone)]
pub struct SupervisorOptions {
  strategy: SupervisorStrategy,
}

impl Default for SupervisorOptions {
  fn default() -> Self {
    Self { strategy: SupervisorStrategy::one_for_one() }
  }
}

impl SupervisorOptions {
  /// Returns the configured strategy.
  #[must_use]
  pub const fn strategy(&self) -> &SupervisorStrategy {
    &self.strategy
  }

  /// Updates the supervisor strategy.
  #[must_use]
  pub fn with_strategy(mut self, strategy: SupervisorStrategy) -> Self {
    self.strategy = strategy;
    self
  }
}
