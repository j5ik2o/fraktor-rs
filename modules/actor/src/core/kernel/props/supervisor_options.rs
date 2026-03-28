use crate::core::kernel::supervision::{SupervisorStrategy, SupervisorStrategyConfig};

#[cfg(test)]
mod tests;

/// Supervisor configuration attached to props.
#[derive(Clone, Debug)]
pub struct SupervisorOptions {
  strategy: SupervisorStrategyConfig,
}

impl SupervisorOptions {
  /// Creates supervisor options from a strategy configuration.
  #[must_use]
  pub const fn new(strategy: SupervisorStrategyConfig) -> Self {
    Self { strategy }
  }

  /// Creates supervisor options from a standard strategy.
  #[must_use]
  pub const fn from_strategy(strategy: SupervisorStrategy) -> Self {
    Self { strategy: SupervisorStrategyConfig::Standard(strategy) }
  }

  /// Returns the configured strategy.
  #[must_use]
  pub const fn strategy(&self) -> &SupervisorStrategyConfig {
    &self.strategy
  }
}

impl Default for SupervisorOptions {
  fn default() -> Self {
    Self::new(SupervisorStrategyConfig::default())
  }
}
