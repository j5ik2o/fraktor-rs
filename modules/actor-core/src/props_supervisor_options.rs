use crate::supervisor_strategy::SupervisorStrategy;

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
    const fn decider(error: &crate::actor_error::ActorError) -> crate::supervisor_strategy::SupervisorDirective {
      match error {
        | crate::actor_error::ActorError::Recoverable(_) => crate::supervisor_strategy::SupervisorDirective::Restart,
        | crate::actor_error::ActorError::Fatal(_) => crate::supervisor_strategy::SupervisorDirective::Stop,
      }
    }

    Self::new(crate::supervisor_strategy::SupervisorStrategy::new(
      crate::supervisor_strategy::SupervisorStrategyKind::OneForOne,
      10,
      core::time::Duration::from_secs(1),
      decider,
    ))
  }
}
