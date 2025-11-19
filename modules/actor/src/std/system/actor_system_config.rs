use alloc::string::String;

use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use crate::{
  core::{
    actor_prim::actor_path::GuardianKind,
    dispatcher::DispatcherConfigGeneric,
    extension::ExtensionInstallers,
    scheduler::{SchedulerConfig, TickDriverConfig},
    system::{ActorRefProviderInstaller, ActorSystemConfigGeneric as CoreActorSystemConfigGeneric, RemotingConfig},
  },
  std::dispatcher::DispatcherConfig,
};

/// Configuration for the actor system.
pub struct ActorSystemConfig {
  inner: CoreActorSystemConfigGeneric<StdToolbox>,
}

impl ActorSystemConfig {
  /// Sets the actor system name.
  #[must_use]
  pub fn with_system_name(mut self, name: impl Into<String>) -> Self {
    self.inner = self.inner.with_system_name(name);
    self
  }

  /// Sets the default guardian segment (`/system` or `/user`).
  #[must_use]
  pub fn with_default_guardian(mut self, guardian: GuardianKind) -> Self {
    self.inner = self.inner.with_default_guardian(guardian);
    self
  }

  /// Enables remoting with the given configuration.
  #[must_use]
  pub fn with_remoting_config(mut self, config: RemotingConfig) -> Self {
    self.inner = self.inner.with_remoting_config(config);
    self
  }

  /// Configures the scheduler used by the runtime.
  #[must_use]
  pub fn with_scheduler_config(mut self, config: SchedulerConfig) -> Self {
    self.inner = self.inner.with_scheduler_config(config);
    self
  }

  /// Sets the tick driver configuration.
  #[must_use]
  pub fn with_tick_driver(mut self, config: TickDriverConfig<StdToolbox>) -> Self {
    self.inner = self.inner.with_tick_driver(config);
    self
  }

  /// Registers extension installers executed after bootstrap.
  #[must_use]
  pub fn with_extension_installers(mut self, installers: ExtensionInstallers<StdToolbox>) -> Self {
    self.inner = self.inner.with_extension_installers(installers);
    self
  }

  /// Registers a custom actor-ref provider installer.
  #[must_use]
  pub fn with_actor_ref_provider_installer<P>(mut self, installer: P) -> Self
  where
    P: ActorRefProviderInstaller<StdToolbox> + 'static, {
    self.inner = self.inner.with_actor_ref_provider_installer(installer);
    self
  }

  /// Sets the default dispatcher configuration used when Props don't specify a dispatcher.
  #[must_use]
  pub fn with_default_dispatcher(mut self, config: DispatcherConfig) -> Self {
    self.inner = self.inner.with_default_dispatcher(config.into_core());
    self
  }

  /// Returns the system name.
  #[must_use]
  pub fn system_name(&self) -> &str {
    &self.inner.system_name()
  }

  /// Returns the default guardian kind.
  #[must_use]
  pub const fn default_guardian(&self) -> GuardianKind {
    self.inner.default_guardian()
  }

  /// Returns the remoting configuration if enabled.
  #[must_use]
  pub const fn remoting_config(&self) -> Option<&RemotingConfig> {
    self.inner.remoting_config()
  }

  /// Returns the scheduler configuration.
  #[must_use]
  pub const fn scheduler_config(&self) -> &SchedulerConfig {
    &self.inner.scheduler_config()
  }

  /// Returns the tick driver configuration if set.
  #[must_use]
  pub const fn tick_driver_config(&self) -> Option<&TickDriverConfig<StdToolbox>> {
    self.inner.tick_driver_config()
  }

  /// Takes the tick driver configuration.
  #[must_use]
  pub const fn take_tick_driver_config(&mut self) -> Option<TickDriverConfig<StdToolbox>> {
    self.inner.take_tick_driver_config()
  }

  /// Returns the extension installers if set.
  #[must_use]
  pub const fn extension_installers(&self) -> Option<&ExtensionInstallers<StdToolbox>> {
    self.inner.extension_installers()
  }

  /// Takes the extension installers.
  #[must_use]
  pub const fn take_extension_installers(&mut self) -> Option<ExtensionInstallers<StdToolbox>> {
    self.inner.take_extension_installers()
  }

  /// Returns the provider installer if set.
  #[must_use]
  pub const fn provider_installer(&self) -> Option<&ArcShared<dyn ActorRefProviderInstaller<StdToolbox>>> {
    self.inner.provider_installer()
  }

  /// Takes the provider installer.
  #[must_use]
  pub const fn take_provider_installer(&mut self) -> Option<ArcShared<dyn ActorRefProviderInstaller<StdToolbox>>> {
    self.inner.take_provider_installer()
  }

  /// Returns the default dispatcher configuration if set.
  #[must_use]
  pub const fn default_dispatcher_config(&self) -> Option<&DispatcherConfigGeneric<StdToolbox>> {
    self.inner.default_dispatcher_config()
  }

  /// Borrows the underlying core props reference.
  #[must_use]
  pub const fn as_core(&self) -> &CoreActorSystemConfigGeneric<StdToolbox> {
    &self.inner
  }

  /// Borrows the underlying core props mutably.
  #[must_use]
  pub const fn as_core_mut(&mut self) -> &mut CoreActorSystemConfigGeneric<StdToolbox> {
    &mut self.inner
  }

  /// Consumes the wrapper and returns the underlying core props.
  #[must_use]
  pub fn into_inner(self) -> CoreActorSystemConfigGeneric<StdToolbox> {
    self.inner
  }
}

impl Default for ActorSystemConfig {
  fn default() -> Self {
    Self { inner: CoreActorSystemConfigGeneric::default() }
  }
}
