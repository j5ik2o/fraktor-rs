use crate::supervision::SupervisorStrategy;

#[cfg(test)]
mod tests;

/// Supervisor configuration attached to props.
#[derive(Clone, Copy, Debug)]
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
    const fn decider(error: &crate::error::ActorError) -> crate::supervision::SupervisorDirective {
      match error {
        | crate::error::ActorError::Recoverable(_) => crate::supervision::SupervisorDirective::Restart,
        | crate::error::ActorError::Fatal(_) => crate::supervision::SupervisorDirective::Stop,
      }
    }

    Self::new(crate::supervision::SupervisorStrategy::new(
      crate::supervision::SupervisorStrategyKind::OneForOne,
      10,
      core::time::Duration::from_secs(1),
      decider,
    ))
  }
}
