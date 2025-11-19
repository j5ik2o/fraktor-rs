//! Actor system configuration API.

use alloc::string::{String, ToString};

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use super::RemotingConfig;
use crate::core::{
  actor_prim::actor_path::GuardianKind as PathGuardianKind,
  extension::ExtensionsConfig,
  scheduler::{SchedulerConfig, TickDriverConfig},
  system::ActorRefProviderInstaller,
};

#[cfg(test)]
mod tests;

/// Configuration for the actor system.
pub struct ActorSystemConfig<TB>
where
  TB: RuntimeToolbox + 'static, {
  system_name:        String,
  default_guardian:   PathGuardianKind,
  remoting_config:    Option<RemotingConfig>,
  scheduler_config:   SchedulerConfig,
  tick_driver_config: Option<TickDriverConfig<TB>>,
  extensions_config:  Option<ExtensionsConfig<TB>>,
  provider_installer: Option<ArcShared<dyn ActorRefProviderInstaller<TB>>>,
}

impl<TB> ActorSystemConfig<TB>
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
  pub fn with_extensions_config(mut self, config: ExtensionsConfig<TB>) -> Self {
    self.extensions_config = Some(config);
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

  /// Returns the system name.
  #[must_use]
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

  /// Returns the extensions configuration if set.
  #[must_use]
  pub const fn extensions_config(&self) -> Option<&ExtensionsConfig<TB>> {
    self.extensions_config.as_ref()
  }

  /// Takes the extensions configuration.
  #[must_use]
  pub const fn take_extensions_config(&mut self) -> Option<ExtensionsConfig<TB>> {
    self.extensions_config.take()
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
}

impl<TB> Default for ActorSystemConfig<TB>
where
  TB: RuntimeToolbox + 'static,
{
  fn default() -> Self {
    Self {
      system_name:        "default-system".to_string(),
      default_guardian:   PathGuardianKind::User,
      remoting_config:    None,
      scheduler_config:   SchedulerConfig::default(),
      tick_driver_config: None,
      extensions_config:  None,
      provider_installer: None,
    }
  }
}
