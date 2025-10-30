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
    const DEFAULT_WITHIN: core::time::Duration = core::time::Duration::from_secs(1);
    const fn decide(_: &crate::actor_error::ActorError) -> crate::supervisor_strategy::SupervisorDirective {
      crate::supervisor_strategy::SupervisorDirective::Restart
    }
    Self::new(SupervisorStrategy::new(
      crate::supervisor_strategy::SupervisorStrategyKind::OneForOne,
      10,
      DEFAULT_WITHIN,
      decide,
    ))
  }
}
