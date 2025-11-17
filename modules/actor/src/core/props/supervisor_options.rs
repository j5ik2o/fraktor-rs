use crate::core::supervision::SupervisorStrategy;

#[cfg(test)]
mod tests;

/// Supervisor configuration attached to props.
#[derive(Clone, Debug)]
pub struct SupervisorOptions {
  strategy: SupervisorStrategy,
}

impl SupervisorOptions {
  /// Creates supervisor options.
  #[must_use]
  pub const fn new(strategy: SupervisorStrategy) -> Self {
    Self { strategy }
  }

  /// Returns the configured strategy.
  #[must_use]
  pub const fn strategy(&self) -> &SupervisorStrategy {
    &self.strategy
  }
}

impl Default for SupervisorOptions {
  fn default() -> Self {
    const fn decider(error: &crate::core::error::ActorError) -> crate::core::supervision::SupervisorDirective {
      match error {
        | crate::core::error::ActorError::Recoverable(_) => crate::core::supervision::SupervisorDirective::Restart,
        | crate::core::error::ActorError::Fatal(_) => crate::core::supervision::SupervisorDirective::Stop,
      }
    }

    Self::new(crate::core::supervision::SupervisorStrategy::new(
      crate::core::supervision::SupervisorStrategyKind::OneForOne,
      10,
      core::time::Duration::from_secs(1),
      decider,
    ))
  }
}
