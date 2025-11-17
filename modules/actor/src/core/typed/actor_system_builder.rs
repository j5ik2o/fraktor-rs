use core::marker::PhantomData;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  config::ActorSystemConfig,
  scheduler::{SchedulerConfig, TickDriverConfig},
  system::{ActorSystemBuildError, ActorSystemBuilder},
  typed::{TypedActorSystemGeneric, TypedPropsGeneric},
};

/// Builder that provisions typed actor systems with an attached tick driver.
pub struct TypedActorSystemBuilder<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + Default + 'static, {
  inner:  ActorSystemBuilder<TB>,
  marker: PhantomData<M>,
}

impl<M, TB> TypedActorSystemBuilder<M, TB>
where
  M: Send + Sync + 'static,
  TB: RuntimeToolbox + Default + 'static,
{
  /// Creates a new typed builder from the provided guardian props.
  #[must_use]
  pub fn new(props: TypedPropsGeneric<M, TB>) -> Self {
    Self { inner: ActorSystemBuilder::new(props.into_untyped()), marker: PhantomData }
  }

  /// Configures the actor system settings applied during bootstrap.
  #[must_use]
  pub fn with_actor_system_config(self, config: ActorSystemConfig<TB>) -> Self {
    Self { inner: self.inner.with_actor_system_config(config), marker: PhantomData }
  }

  /// Configures the scheduler parameters.
  #[must_use]
  pub fn with_scheduler_config(self, config: SchedulerConfig) -> Self {
    Self { inner: self.inner.with_scheduler_config(config), marker: PhantomData }
  }

  /// Sets the tick driver configuration.
  #[must_use]
  pub fn with_tick_driver(self, config: TickDriverConfig<TB>) -> Self {
    Self { inner: self.inner.with_tick_driver(config), marker: PhantomData }
  }

  /// Builds the typed actor system and provisions the tick driver.
  ///
  /// # Errors
  ///
  /// Returns an error if the underlying actor system builder fails.
  /// See [`ActorSystemBuilder::build`] for details.
  pub fn build(self) -> Result<TypedActorSystemGeneric<M, TB>, ActorSystemBuildError> {
    let system = self.inner.build()?;
    Ok(TypedActorSystemGeneric::from_untyped(system))
  }
}
