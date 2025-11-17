use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use super::base::ActorSystem;
use crate::{
  core::{
    config::ActorSystemConfig,
    scheduler::{SchedulerConfig, TickDriverConfig},
    system::{ActorSystemBuildError, ActorSystemBuilder as CoreActorSystemBuilder},
  },
  std::props::Props,
};

/// Builder specialized for std actor systems.
pub struct ActorSystemBuilder {
  inner: CoreActorSystemBuilder<StdToolbox>,
}

impl ActorSystemBuilder {
  /// Creates a builder using the provided guardian props.
  #[must_use]
  pub fn new(props: Props) -> Self {
    Self { inner: CoreActorSystemBuilder::new(props.into_inner()) }
  }

  /// Overrides the actor system configuration.
  #[must_use]
  pub fn with_actor_system_config(mut self, config: ActorSystemConfig<StdToolbox>) -> Self {
    self.inner = self.inner.with_actor_system_config(config);
    self
  }

  /// Overrides the scheduler configuration.
  #[must_use]
  pub fn with_scheduler_config(mut self, config: SchedulerConfig) -> Self {
    self.inner = self.inner.with_scheduler_config(config);
    self
  }

  /// Assigns the tick driver configuration to bootstrap.
  #[must_use]
  pub fn with_tick_driver(mut self, config: TickDriverConfig<StdToolbox>) -> Self {
    self.inner = self.inner.with_tick_driver(config);
    self
  }

  /// Builds the std actor system, provisioning the configured tick driver.
  ///
  /// # Errors
  ///
  /// Returns [`ActorSystemBuildError`] when actor bootstrap or tick driver provisioning fails.
  pub fn build(self) -> Result<ActorSystem, ActorSystemBuildError> {
    self.inner.build().map(ActorSystem::from_core)
  }
}
