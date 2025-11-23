//! Actor system configuration API.

use alloc::string::{String, ToString};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use crate::core::{
  actor_prim::actor_path::GuardianKind as PathGuardianKind,
  dispatcher::DispatcherConfigGeneric,
  extension::ExtensionInstallers,
  scheduler::{SchedulerConfig, TickDriverConfig},
  system::{ActorRefProviderInstaller, RemotingConfig},
};

#[cfg(test)]
mod tests;

/// Configuration for the actor system.
pub struct ActorSystemConfigGeneric<TB>
where
  TB: RuntimeToolbox + 'static, {
  system_name:               String,
  default_guardian:          PathGuardianKind,
  remoting_config:           Option<RemotingConfig>,
  scheduler_config:          SchedulerConfig,
  tick_driver_config:        Option<TickDriverConfig<TB>>,
  extension_installers:      Option<ExtensionInstallers<TB>>,
  provider_installer:        Option<ArcShared<dyn ActorRefProviderInstaller<TB>>>,
  default_dispatcher_config: Option<DispatcherConfigGeneric<TB>>,
}

/// Type alias for [ActorSystemConfigGeneric] with the default [NoStdToolbox].
pub type ActorSystemConfig = ActorSystemConfigGeneric<NoStdToolbox>;

impl<TB> ActorSystemConfigGeneric<TB>
where
  TB: RuntimeToolbox + 'static,
{
  /// Sets the actor system name.
  #[must_use]
  pub fn with_system_name(mut self, name: impl Into<String>) -> Self {
    self.system_name = name.into();
    self
  }

  /// Sets the default guardian segment (`/system` or `/user`).
  #[must_use]
  pub const fn with_default_guardian(mut self, guardian: PathGuardianKind) -> Self {
    self.default_guardian = guardian;
    self
  }

  /// Enables remoting with the given configuration.
  #[must_use]
  pub fn with_remoting_config(mut self, config: RemotingConfig) -> Self {
    self.remoting_config = Some(config);
    self
  }

  /// Configures the scheduler used by the runtime.
  #[must_use]
  pub const fn with_scheduler_config(mut self, config: SchedulerConfig) -> Self {
    self.scheduler_config = config;
    self
  }

  /// Sets the tick driver configuration.
  #[must_use]
  pub fn with_tick_driver(mut self, config: TickDriverConfig<TB>) -> Self {
    self.tick_driver_config = Some(config);
    self
  }

  /// Registers extension installers executed after bootstrap.
  #[must_use]
  pub fn with_extension_installers(mut self, installers: ExtensionInstallers<TB>) -> Self {
    self.extension_installers = Some(installers);
    self
  }

  /// Registers a custom actor-ref provider installer.
  #[must_use]
  pub fn with_actor_ref_provider_installer<P>(mut self, installer: P) -> Self
  where
    P: ActorRefProviderInstaller<TB> + 'static, {
    self.provider_installer = Some(ArcShared::new(installer));
    self
  }

  /// Sets the default dispatcher configuration used when Props don't specify a dispatcher.
  #[must_use]
  pub fn with_default_dispatcher(mut self, config: DispatcherConfigGeneric<TB>) -> Self {
    self.default_dispatcher_config = Some(config);
    self
  }

  /// Returns the system name.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // String の Deref が const でないため const fn にできない
  pub fn system_name(&self) -> &str {
    &self.system_name
  }

  /// Returns the default guardian kind.
  #[must_use]
  pub const fn default_guardian(&self) -> PathGuardianKind {
    self.default_guardian
  }

  /// Returns the remoting configuration if enabled.
  #[must_use]
  pub const fn remoting_config(&self) -> Option<&RemotingConfig> {
    self.remoting_config.as_ref()
  }

  /// Returns the scheduler configuration.
  #[must_use]
  pub const fn scheduler_config(&self) -> &SchedulerConfig {
    &self.scheduler_config
  }

  /// Returns the tick driver configuration if set.
  #[must_use]
  pub const fn tick_driver_config(&self) -> Option<&TickDriverConfig<TB>> {
    self.tick_driver_config.as_ref()
  }

  /// Takes the tick driver configuration.
  #[must_use]
  pub const fn take_tick_driver_config(&mut self) -> Option<TickDriverConfig<TB>> {
    self.tick_driver_config.take()
  }

  /// Returns the extension installers if set.
  #[must_use]
  pub const fn extension_installers(&self) -> Option<&ExtensionInstallers<TB>> {
    self.extension_installers.as_ref()
  }

  /// Takes the extension installers.
  #[must_use]
  pub const fn take_extension_installers(&mut self) -> Option<ExtensionInstallers<TB>> {
    self.extension_installers.take()
  }

  /// Returns the provider installer if set.
  #[must_use]
  pub const fn provider_installer(&self) -> Option<&ArcShared<dyn ActorRefProviderInstaller<TB>>> {
    self.provider_installer.as_ref()
  }

  /// Takes the provider installer.
  #[must_use]
  pub const fn take_provider_installer(&mut self) -> Option<ArcShared<dyn ActorRefProviderInstaller<TB>>> {
    self.provider_installer.take()
  }

  /// Returns the default dispatcher configuration if set.
  #[must_use]
  pub const fn default_dispatcher_config(&self) -> Option<&DispatcherConfigGeneric<TB>> {
    self.default_dispatcher_config.as_ref()
  }
}

impl<TB> Default for ActorSystemConfigGeneric<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn default() -> Self {
    Self {
      system_name:               "default-system".to_string(),
      default_guardian:          PathGuardianKind::User,
      remoting_config:           None,
      scheduler_config:          SchedulerConfig::default(),
      tick_driver_config:        None,
      extension_installers:      None,
      provider_installer:        None,
      default_dispatcher_config: None,
    }
  }
}
