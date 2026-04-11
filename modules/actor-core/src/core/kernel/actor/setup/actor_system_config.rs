//! Actor system configuration API.

use alloc::{
  boxed::Box,
  string::{String, ToString},
};
use core::time::Duration;

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::{
  actor::{
    actor_path::GuardianKind as PathGuardianKind,
    actor_ref_provider::ActorRefProviderInstaller,
    extension::ExtensionInstallers,
    props::MailboxConfig,
    scheduler::{SchedulerConfig, tick_driver::TickDriverConfig},
  },
  dispatch::{
    dispatcher::{Dispatchers, MessageDispatcherConfigurator},
    mailbox::Mailboxes,
  },
  system::{
    remote::RemotingConfig,
    shared_factory::{ActorSharedFactory, BuiltinSpinSharedFactory},
  },
};

#[cfg(test)]
mod tests;

/// Configuration for the actor system.
pub struct ActorSystemConfig {
  system_name:          String,
  default_guardian:     PathGuardianKind,
  remoting_config:      Option<RemotingConfig>,
  scheduler_config:     SchedulerConfig,
  tick_driver_config:   Option<TickDriverConfig>,
  extension_installers: Option<ExtensionInstallers>,
  provider_installer:   Option<ArcShared<dyn ActorRefProviderInstaller>>,
  shared_factory:       ArcShared<dyn ActorSharedFactory>,
  dispatchers:          Dispatchers,
  mailboxes:            Mailboxes,
  start_time:           Option<Duration>,
}

impl ActorSystemConfig {
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

  /// Sets or clears the remoting configuration.
  #[must_use]
  pub fn with_remoting_config(mut self, config: impl Into<Option<RemotingConfig>>) -> Self {
    self.remoting_config = config.into();
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
  pub fn with_tick_driver(mut self, config: TickDriverConfig) -> Self {
    self.tick_driver_config = Some(config);
    self
  }

  /// Registers extension installers executed after bootstrap.
  #[must_use]
  pub fn with_extension_installers(mut self, installers: ExtensionInstallers) -> Self {
    self.extension_installers = Some(installers);
    self
  }

  /// Registers a custom actor-ref provider installer.
  #[must_use]
  pub fn with_actor_ref_provider_installer<P>(mut self, installer: P) -> Self
  where
    P: ActorRefProviderInstaller + 'static, {
    self.provider_installer = Some(ArcShared::new(installer));
    self
  }

  /// Overrides the actor-system scoped shared factory.
  #[must_use]
  pub fn with_shared_factory<P>(mut self, provider: P) -> Self
  where
    P: ActorSharedFactory + 'static, {
    self.shared_factory = ArcShared::new(provider);
    self.dispatchers.replace_default_inline_with_provider(&self.shared_factory);
    self
  }

  /// Registers a dispatcher configurator under the supplied id.
  ///
  /// `ActorSystemConfig::default()` seeds the registry with an
  /// `InlineExecutor`-backed configurator under the default id; production
  /// users override the entry by calling this method with a configurator
  /// that uses a real executor (Tokio, threaded, pinned, etc.).
  #[must_use]
  pub fn with_dispatcher_configurator(
    mut self,
    id: impl Into<String>,
    configurator: ArcShared<Box<dyn MessageDispatcherConfigurator>>,
  ) -> Self {
    self.dispatchers.register_or_update(id, configurator);
    self
  }

  /// Registers or updates a mailbox configuration.
  #[must_use]
  pub fn with_mailbox(mut self, id: impl Into<String>, config: MailboxConfig) -> Self {
    self.mailboxes.register_or_update(id, config);
    self
  }

  /// Sets the start time of the actor system (epoch-relative duration).
  ///
  /// In `no_std` environments the caller must inject the current time.
  /// Corresponds to Pekko's `ActorSystem.startTime`.
  #[must_use]
  pub fn with_start_time(mut self, start_time: impl Into<Option<Duration>>) -> Self {
    self.start_time = start_time.into();
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
  pub const fn tick_driver_config(&self) -> Option<&TickDriverConfig> {
    self.tick_driver_config.as_ref()
  }

  /// Takes the tick driver configuration.
  #[must_use]
  pub const fn take_tick_driver_config(&mut self) -> Option<TickDriverConfig> {
    self.tick_driver_config.take()
  }

  /// Returns the extension installers if set.
  #[must_use]
  pub const fn extension_installers(&self) -> Option<&ExtensionInstallers> {
    self.extension_installers.as_ref()
  }

  /// Takes the extension installers.
  #[must_use]
  pub const fn take_extension_installers(&mut self) -> Option<ExtensionInstallers> {
    self.extension_installers.take()
  }

  /// Returns the provider installer if set.
  #[must_use]
  pub const fn provider_installer(&self) -> Option<&ArcShared<dyn ActorRefProviderInstaller>> {
    self.provider_installer.as_ref()
  }

  /// Takes the provider installer.
  #[must_use]
  pub const fn take_provider_installer(&mut self) -> Option<ArcShared<dyn ActorRefProviderInstaller>> {
    self.provider_installer.take()
  }

  /// Returns the actor-system scoped shared factory.
  #[must_use]
  pub const fn shared_factory(&self) -> &ArcShared<dyn ActorSharedFactory> {
    &self.shared_factory
  }

  /// Returns the dispatcher registry configured for the system.
  #[must_use]
  pub const fn dispatchers(&self) -> &Dispatchers {
    &self.dispatchers
  }

  /// Returns the mailbox registry configured for the system.
  #[must_use]
  pub const fn mailboxes(&self) -> &Mailboxes {
    &self.mailboxes
  }

  /// Returns the configured start time, or `None` if not set.
  ///
  /// Corresponds to Pekko's `ActorSystem.startTime`.
  #[must_use]
  pub const fn start_time(&self) -> Option<Duration> {
    self.start_time
  }
}

impl Default for ActorSystemConfig {
  fn default() -> Self {
    let lock_provider: ArcShared<dyn ActorSharedFactory> = ArcShared::new(BuiltinSpinSharedFactory::new());
    let mut dispatchers = Dispatchers::new();
    dispatchers.ensure_default_inline(&lock_provider);
    let mut mailboxes = Mailboxes::new();
    mailboxes.ensure_default();
    Self {
      system_name: "default-system".to_string(),
      default_guardian: PathGuardianKind::User,
      remoting_config: None,
      scheduler_config: SchedulerConfig::default(),
      tick_driver_config: None,
      extension_installers: None,
      provider_installer: None,
      shared_factory: lock_provider,
      dispatchers,
      mailboxes,
      start_time: None,
    }
  }
}
