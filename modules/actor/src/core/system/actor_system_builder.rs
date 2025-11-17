use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{actor_system_build_error::ActorSystemBuildError, base::ActorSystemGeneric};
use crate::core::{
  config::ActorSystemConfig,
  props::PropsGeneric,
  scheduler::{SchedulerConfig, TickDriverConfig},
};

/// Builds [`ActorSystemGeneric`] instances with a configured tick driver.
pub struct ActorSystemBuilder<TB>
where
  TB: RuntimeToolbox + Default + 'static, {
  state: BuilderState<TB>,
}

struct BuilderState<TB>
where
  TB: RuntimeToolbox + 'static, {
  props:               PropsGeneric<TB>,
  actor_system_config: ActorSystemConfig<TB>,
}

impl<TB> BuilderState<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn new(props: PropsGeneric<TB>) -> Self {
    Self { props, actor_system_config: ActorSystemConfig::default() }
  }
}

impl<TB> ActorSystemBuilder<TB>
where
  TB: RuntimeToolbox + Default + 'static,
{
  /// Creates a builder using the provided user guardian props.
  #[must_use]
  pub fn new(props: PropsGeneric<TB>) -> Self {
    Self { state: BuilderState::new(props) }
  }

  /// Configures the actor system settings applied during bootstrap.
  #[must_use]
  pub fn with_actor_system_config(mut self, config: ActorSystemConfig<TB>) -> Self {
    self.state.actor_system_config = config;
    self
  }

  /// Configures the scheduler used by the runtime.
  #[must_use]
  pub fn with_scheduler_config(mut self, config: SchedulerConfig) -> Self {
    self.state.actor_system_config = self.state.actor_system_config.with_scheduler_config(config);
    self
  }

  /// Sets the tick driver configuration.
  #[must_use]
  pub fn with_tick_driver(mut self, config: TickDriverConfig<TB>) -> Self {
    self.state.actor_system_config = self.state.actor_system_config.with_tick_driver(config);
    self
  }

  /// Builds the actor system and provisions the configured tick driver.
  ///
  /// # Errors
  ///
  /// Returns an error if:
  /// - The tick driver configuration is missing
  /// - Actor system initialization fails
  /// - Tick driver provisioning fails
  #[allow(unused_mut)]
  pub fn build(self) -> Result<ActorSystemGeneric<TB>, ActorSystemBuildError> {
    let BuilderState { props, mut actor_system_config } = self.state;

    // Ensure tick driver configuration is present
    if actor_system_config.tick_driver_config().is_none() {
      return Err(ActorSystemBuildError::MissingTickDriver);
    }

    // Special handling for ManualTest driver in test mode
    #[cfg(any(test, feature = "test-support"))]
    if let Some(tick_driver_config) = actor_system_config.tick_driver_config()
      && matches!(tick_driver_config, TickDriverConfig::ManualTest(_))
      && !actor_system_config.scheduler_config().runner_api_enabled()
    {
      let new_scheduler_config = actor_system_config.scheduler_config().with_runner_api_enabled(true);
      actor_system_config = actor_system_config.with_scheduler_config(new_scheduler_config);
    }

    // Create actor system with full configuration
    // The scheduler context and tick driver runtime will be installed automatically
    ActorSystemGeneric::new_with_config(&props, &actor_system_config).map_err(ActorSystemBuildError::Spawn)
  }
}
