//! Actor-system setup facade composed from bootstrap and runtime config.

#[cfg(test)]
mod tests;

use crate::core::kernel::{
  actor::{
    actor_ref_provider::ActorRefProviderInstaller,
    extension::ExtensionInstallers,
    props::MailboxConfig,
    scheduler::{SchedulerConfig, tick_driver::TickDriverConfig},
    setup::{ActorSystemConfig, BootstrapSetup},
  },
  dispatch::dispatcher::DispatcherConfig,
};

/// Pekko-compatible setup aggregate backed by [`ActorSystemConfig`].
pub struct ActorSystemSetup {
  config: ActorSystemConfig,
}

impl ActorSystemSetup {
  /// Creates a new actor-system setup from the provided bootstrap setup.
  #[must_use]
  pub fn new(bootstrap: BootstrapSetup) -> Self {
    Self { config: bootstrap.into_actor_system_config() }
  }

  /// Replaces the bootstrap portion of this setup.
  ///
  /// Runtime settings such as dispatcher, mailbox, extension, provider,
  /// scheduler, and tick driver configuration are preserved.
  #[must_use]
  pub fn with_bootstrap_setup(self, bootstrap: BootstrapSetup) -> Self {
    let config = self.config;
    let bootstrap = bootstrap.into_actor_system_config();
    let config = config
      .with_system_name(bootstrap.system_name())
      .with_default_guardian(bootstrap.default_guardian());
    let config = match bootstrap.remoting_config() {
      | Some(remoting) => config.with_remoting_config(remoting.clone()),
      | None => config,
    };
    let config = match bootstrap.start_time() {
      | Some(start_time) => config.with_start_time(start_time),
      | None => config,
    };
    Self { config }
  }

  /// Configures the runtime scheduler.
  #[must_use]
  pub fn with_scheduler_config(self, config: SchedulerConfig) -> Self {
    Self { config: self.config.with_scheduler_config(config) }
  }

  /// Configures the tick driver.
  #[must_use]
  pub fn with_tick_driver(self, config: TickDriverConfig) -> Self {
    Self { config: self.config.with_tick_driver(config) }
  }

  /// Registers extension installers.
  #[must_use]
  pub fn with_extension_installers(self, installers: ExtensionInstallers) -> Self {
    Self { config: self.config.with_extension_installers(installers) }
  }

  /// Registers a custom actor-ref provider installer.
  #[must_use]
  pub fn with_actor_ref_provider_installer<P>(self, installer: P) -> Self
  where
    P: ActorRefProviderInstaller + 'static, {
    Self { config: self.config.with_actor_ref_provider_installer(installer) }
  }

  /// Sets the default dispatcher configuration.
  #[must_use]
  pub fn with_default_dispatcher(self, config: DispatcherConfig) -> Self {
    Self { config: self.config.with_default_dispatcher(config) }
  }

  /// Registers or updates a dispatcher configuration.
  #[must_use]
  pub fn with_dispatcher(self, id: impl Into<alloc::string::String>, config: DispatcherConfig) -> Self {
    Self { config: self.config.with_dispatcher(id, config) }
  }

  /// Registers or updates a mailbox configuration.
  #[must_use]
  pub fn with_mailbox(self, id: impl Into<alloc::string::String>, config: MailboxConfig) -> Self {
    Self { config: self.config.with_mailbox(id, config) }
  }

  /// Returns the underlying actor-system config.
  #[must_use]
  pub const fn as_actor_system_config(&self) -> &ActorSystemConfig {
    &self.config
  }

  /// Consumes the setup and returns the underlying actor-system config.
  #[must_use]
  pub fn into_actor_system_config(self) -> ActorSystemConfig {
    self.config
  }
}

impl Default for ActorSystemSetup {
  fn default() -> Self {
    Self::new(BootstrapSetup::default())
  }
}

impl From<ActorSystemSetup> for ActorSystemConfig {
  fn from(value: ActorSystemSetup) -> Self {
    value.into_actor_system_config()
  }
}
