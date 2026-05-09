//! Actor system configuration API.

use alloc::{
  boxed::Box,
  collections::BTreeMap,
  string::{String, ToString},
};
use core::time::Duration;

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::{
  actor::{
    actor_path::GuardianKind as PathGuardianKind,
    actor_ref_provider::ActorRefProviderInstaller,
    extension::ExtensionInstallers,
    invoke_guard::{InvokeGuardFactory, NoopInvokeGuardFactory},
    scheduler::{SchedulerConfig, tick_driver::TickDriver},
    setup::CircuitBreakerConfig,
  },
  dispatch::{
    dispatcher::{Dispatchers, MessageDispatcherFactory},
    mailbox::{MailboxClock, MailboxFactory, Mailboxes},
  },
  system::remote::RemotingConfig,
};

#[cfg(test)]
mod tests;

/// Configuration for the actor system.
pub struct ActorSystemConfig {
  system_name: String,
  default_guardian: PathGuardianKind,
  remoting_config: Option<RemotingConfig>,
  scheduler_config: SchedulerConfig,
  tick_driver: Option<Box<dyn TickDriver>>,
  extension_installers: Option<ExtensionInstallers>,
  provider_installer: Option<ArcShared<dyn ActorRefProviderInstaller>>,
  invoke_guard_factory: Option<ArcShared<Box<dyn InvokeGuardFactory>>>,
  dispatchers: Dispatchers,
  mailboxes: Mailboxes,
  /// Optional monotonic clock installed into [`MailboxSharedSet`] during
  /// `SystemState::build_from_owned_config`. `None` leaves deadline
  /// enforcement disabled (Pekko `isThroughputDeadlineTimeDefined = false`
  /// equivalent). std adaptors populate this with [`MailboxClock`] backed by
  /// `Instant::now()` so every system created through the adaptor gets
  /// production-grade deadline enforcement.
  mailbox_clock: Option<MailboxClock>,
  default_circuit_breaker_config: CircuitBreakerConfig,
  named_circuit_breaker_config: BTreeMap<String, CircuitBreakerConfig>,
  start_time: Option<Duration>,
}

impl ActorSystemConfig {
  /// Creates a new configuration with the provided tick driver.
  #[must_use]
  pub fn new(driver: impl TickDriver + 'static) -> Self {
    Self::default().with_tick_driver(driver)
  }

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

  /// Sets the tick driver.
  #[must_use]
  pub fn with_tick_driver(mut self, driver: impl TickDriver + 'static) -> Self {
    self.tick_driver = Some(Box::new(driver));
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

  /// Registers a custom invoke-guard factory.
  #[must_use]
  pub fn with_invoke_guard_factory(mut self, factory: ArcShared<Box<dyn InvokeGuardFactory>>) -> Self {
    self.invoke_guard_factory = Some(factory);
    self
  }

  /// Registers a dispatcher configurator under the supplied id.
  ///
  /// `ActorSystemConfig::default()` seeds the registry with an
  /// `InlineExecutor`-backed configurator under the default id; production
  /// users override the entry by calling this method with a configurator
  /// that uses a real executor (Tokio, threaded, pinned, etc.).
  #[must_use]
  pub fn with_dispatcher_factory(
    mut self,
    id: impl Into<String>,
    configurator: ArcShared<Box<dyn MessageDispatcherFactory>>,
  ) -> Self {
    self.dispatchers.register_or_update(id, configurator);
    self
  }

  /// Registers or updates a mailbox factory under the supplied id.
  ///
  /// Accepts any [`MailboxFactory`] implementation. Since
  /// [`MailboxConfig`] implements [`MailboxFactory`], existing callers can
  /// continue to pass a `MailboxConfig` directly; custom factory types can
  /// also be plugged in to override queue construction and metadata.
  #[must_use]
  pub fn with_mailbox(mut self, id: impl Into<String>, factory: impl MailboxFactory + 'static) -> Self {
    self.mailboxes.register_or_update(id, factory);
    self
  }

  /// Replaces the default circuit-breaker configuration.
  #[must_use]
  pub const fn with_default_circuit_breaker_config(mut self, config: CircuitBreakerConfig) -> Self {
    self.default_circuit_breaker_config = config;
    self
  }

  /// Registers circuit-breaker configuration for a named logical id.
  #[must_use]
  pub fn with_named_circuit_breaker_config(mut self, id: impl Into<String>, config: CircuitBreakerConfig) -> Self {
    self.named_circuit_breaker_config.insert(id.into(), config);
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

  /// Returns `true` if a tick driver has been set.
  #[must_use]
  pub const fn has_tick_driver(&self) -> bool {
    self.tick_driver.is_some()
  }

  /// Takes the tick driver out of the configuration.
  #[must_use]
  pub fn take_tick_driver(&mut self) -> Option<Box<dyn TickDriver>> {
    self.tick_driver.take()
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

  /// Returns the configured invoke-guard factory or the no-op default.
  #[must_use]
  pub fn invoke_guard_factory(&self) -> ArcShared<Box<dyn InvokeGuardFactory>> {
    self.invoke_guard_factory.clone().unwrap_or_else(NoopInvokeGuardFactory::shared)
  }

  /// Takes the invoke-guard factory out of the configuration, falling back to the no-op default.
  #[must_use]
  pub fn take_invoke_guard_factory(&mut self) -> ArcShared<Box<dyn InvokeGuardFactory>> {
    self.invoke_guard_factory.take().unwrap_or_else(NoopInvokeGuardFactory::shared)
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

  /// Returns the default circuit-breaker configuration.
  #[must_use]
  pub const fn default_circuit_breaker_config(&self) -> CircuitBreakerConfig {
    self.default_circuit_breaker_config
  }

  /// Returns the named circuit-breaker overrides.
  #[must_use]
  pub const fn named_circuit_breaker_config(&self) -> &BTreeMap<String, CircuitBreakerConfig> {
    &self.named_circuit_breaker_config
  }

  /// Resolves circuit-breaker configuration for `id`, falling back to the default.
  #[must_use]
  pub fn circuit_breaker_config(&self, id: &str) -> CircuitBreakerConfig {
    self.named_circuit_breaker_config.get(id).copied().unwrap_or(self.default_circuit_breaker_config)
  }

  /// Returns the configured start time, or `None` if not set.
  ///
  /// Corresponds to Pekko's `ActorSystem.startTime`.
  #[must_use]
  pub const fn start_time(&self) -> Option<Duration> {
    self.start_time
  }

  /// Sets the monotonic mailbox clock used for throughput deadline enforcement.
  ///
  /// Called by std / embedded adaptors during `ActorSystemConfig` construction
  /// so that every `ActorSystem` built from this config picks up the clock via
  /// [`ActorSystemConfig::take_mailbox_clock`] in `SystemState::build_from_owned_config`.
  /// Passing `None` disables deadline enforcement.
  #[must_use]
  pub fn with_mailbox_clock(mut self, clock: impl Into<Option<MailboxClock>>) -> Self {
    self.mailbox_clock = clock.into();
    self
  }

  /// Returns a clone of the installed mailbox clock, if any.
  #[must_use]
  pub fn mailbox_clock(&self) -> Option<MailboxClock> {
    self.mailbox_clock.clone()
  }

  /// Consumes the installed mailbox clock, leaving `None` in its place.
  ///
  /// Called by [`SystemState::build_from_owned_config`] during bootstrap to
  /// hand the clock off to the mailbox lock bundle.
  pub(crate) fn take_mailbox_clock(&mut self) -> Option<MailboxClock> {
    self.mailbox_clock.take()
  }
}

impl Default for ActorSystemConfig {
  fn default() -> Self {
    let mut dispatchers = Dispatchers::new();
    dispatchers.ensure_default_inline();
    let mut mailboxes = Mailboxes::new();
    mailboxes.ensure_default();
    Self {
      system_name: "default-system".to_string(),
      default_guardian: PathGuardianKind::User,
      remoting_config: None,
      scheduler_config: SchedulerConfig::default(),
      tick_driver: None,
      extension_installers: None,
      provider_installer: None,
      invoke_guard_factory: None,
      dispatchers,
      mailboxes,
      mailbox_clock: None,
      default_circuit_breaker_config: CircuitBreakerConfig::default(),
      named_circuit_breaker_config: BTreeMap::new(),
      start_time: None,
    }
  }
}
