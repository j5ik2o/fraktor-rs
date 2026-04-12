//! Actor-system setup facade composed from bootstrap and runtime config.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String};

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::{
  actor::{
    ActorCellStateSharedFactory, ActorSharedLockFactory, ReceiveTimeoutStateSharedFactory,
    actor_ref::ActorRefSenderSharedFactory,
    actor_ref_provider::{ActorRefProviderHandleSharedFactory, ActorRefProviderInstaller, LocalActorRefProvider},
    extension::ExtensionInstallers,
    messaging::{AskResult, message_invoker::MessageInvokerSharedFactory},
    props::MailboxConfig,
    scheduler::{SchedulerConfig, tick_driver::TickDriverConfig},
    setup::{ActorSystemConfig, BootstrapSetup},
  },
  dispatch::dispatcher::{
    ExecutorSharedFactory, MessageDispatcherConfigurator, MessageDispatcherSharedFactory, SharedMessageQueueFactory,
  },
  event::stream::{EventStreamSharedFactory, EventStreamSubscriberSharedFactory},
  system::shared_factory::MailboxSharedSetFactory,
  util::futures::ActorFutureSharedFactory,
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
    let config = config.with_system_name(bootstrap.system_name()).with_default_guardian(bootstrap.default_guardian());
    let config = config.with_remoting_config(bootstrap.remoting_config().cloned());
    let config = config.with_start_time(bootstrap.start_time());
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

  /// Overrides the actor-system scoped shared factory.
  #[must_use]
  pub fn with_shared_factory<P>(self, provider: P) -> Self
  where
    P: ExecutorSharedFactory
      + MessageDispatcherSharedFactory
      + SharedMessageQueueFactory
      + ActorRefSenderSharedFactory
      + ActorSharedLockFactory
      + ActorCellStateSharedFactory
      + ReceiveTimeoutStateSharedFactory
      + MessageInvokerSharedFactory
      + ActorFutureSharedFactory<AskResult>
      + ActorRefProviderHandleSharedFactory<LocalActorRefProvider>
      + EventStreamSharedFactory
      + EventStreamSubscriberSharedFactory
      + MailboxSharedSetFactory
      + 'static, {
    Self { config: self.config.with_shared_factory(provider) }
  }

  /// Registers a dispatcher configurator under the supplied id.
  #[must_use]
  pub fn with_dispatcher_configurator(
    self,
    id: impl Into<String>,
    configurator: ArcShared<Box<dyn MessageDispatcherConfigurator>>,
  ) -> Self {
    Self { config: self.config.with_dispatcher_configurator(id, configurator) }
  }

  /// Registers or updates a mailbox configuration.
  #[must_use]
  pub fn with_mailbox(self, id: impl Into<String>, config: MailboxConfig) -> Self {
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
