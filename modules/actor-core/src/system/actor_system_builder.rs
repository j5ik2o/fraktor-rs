use fraktor_utils_core_rs::sync::ArcShared;

use super::{actor_system_build_error::ActorSystemBuildError, base::ActorSystemGeneric};
use crate::{
  RuntimeToolbox,
  config::ActorSystemConfig,
  props::PropsGeneric,
  scheduler::{SchedulerConfig, SchedulerContext, TickDriverBootstrap, TickDriverConfig, TickDriverError},
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
  props:            PropsGeneric<TB>,
  actor_config:     ActorSystemConfig,
  scheduler_config: SchedulerConfig,
  tick_driver:      Option<TickDriverConfig<TB>>,
  toolbox:          Option<TB>,
}

impl<TB> BuilderState<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn new(props: PropsGeneric<TB>) -> Self {
    Self {
      props,
      actor_config: ActorSystemConfig::default(),
      scheduler_config: SchedulerConfig::default(),
      tick_driver: None,
      toolbox: None,
    }
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
  pub fn with_actor_system_config(mut self, config: ActorSystemConfig) -> Self {
    self.state.actor_config = config;
    self
  }

  /// Configures the scheduler used by the runtime.
  #[must_use]
  pub const fn with_scheduler_config(mut self, config: SchedulerConfig) -> Self {
    self.state.scheduler_config = config;
    self
  }

  /// Sets the runtime toolbox used to construct the scheduler context.
  #[must_use]
  pub fn with_toolbox(mut self, toolbox: TB) -> Self {
    self.state.toolbox = Some(toolbox);
    self
  }

  /// Sets the tick driver configuration.
  #[must_use]
  pub fn with_tick_driver(mut self, config: TickDriverConfig<TB>) -> Self {
    self.state.tick_driver = Some(config);
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
    let BuilderState { props, actor_config, mut scheduler_config, tick_driver, toolbox } = self.state;
    let tick_driver = tick_driver.ok_or(ActorSystemBuildError::MissingTickDriver)?;

    #[cfg(any(test, feature = "test-support"))]
    if matches!(tick_driver, TickDriverConfig::ManualTest(_)) && !scheduler_config.runner_api_enabled() {
      scheduler_config = scheduler_config.with_runner_api_enabled(true);
    }

    let system = ActorSystemGeneric::new_with_config(&props, &actor_config).map_err(ActorSystemBuildError::Spawn)?;

    let event_stream = system.state().event_stream();
    let toolbox = toolbox.unwrap_or_else(TB::default);
    let context = SchedulerContext::with_event_stream(toolbox, scheduler_config, event_stream);
    system.state().install_scheduler_context(ArcShared::new(context));

    let ctx =
      system.scheduler_context().ok_or(ActorSystemBuildError::TickDriver(TickDriverError::HandleUnavailable))?;
    let runtime = TickDriverBootstrap::provision(&tick_driver, &ctx).map_err(ActorSystemBuildError::TickDriver)?;
    system.state().install_tick_driver_runtime(runtime);

    Ok(system)
  }
}
