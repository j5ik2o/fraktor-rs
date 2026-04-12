//! Actor system configuration API.

use alloc::{
  boxed::Box,
  string::{String, ToString},
};
use core::time::Duration;

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::{
  actor::{
    ActorCellStateSharedFactory, ActorSharedLockFactory, ReceiveTimeoutStateSharedFactory,
    actor_path::GuardianKind as PathGuardianKind,
    actor_ref::ActorRefSenderSharedFactory,
    actor_ref_provider::{ActorRefProviderHandleSharedFactory, ActorRefProviderInstaller, LocalActorRefProvider},
    context_pipe::ContextPipeWakerHandleSharedFactory,
    extension::ExtensionInstallers,
    messaging::{AskResult, message_invoker::MessageInvokerSharedFactory},
    props::MailboxConfig,
    scheduler::{
      SchedulerConfig,
      tick_driver::{TickDriverConfig, TickDriverControlSharedFactory},
    },
  },
  dispatch::{
    dispatcher::{
      Dispatchers, ExecutorSharedFactory, MessageDispatcherConfigurator, MessageDispatcherSharedFactory,
      SharedMessageQueueFactory,
    },
    mailbox::Mailboxes,
  },
  event::stream::{EventStreamSharedFactory, EventStreamSubscriberSharedFactory},
  system::{
    remote::RemotingConfig,
    shared_factory::{BuiltinSpinSharedFactory, MailboxSharedSetFactory},
  },
  util::futures::ActorFutureSharedFactory,
};

#[cfg(test)]
mod tests;

/// Configuration for the actor system.
pub struct ActorSystemConfig {
  system_name: String,
  default_guardian: PathGuardianKind,
  remoting_config: Option<RemotingConfig>,
  scheduler_config: SchedulerConfig,
  tick_driver_config: Option<TickDriverConfig>,
  extension_installers: Option<ExtensionInstallers>,
  provider_installer: Option<ArcShared<dyn ActorRefProviderInstaller>>,
  executor_shared_factory: ArcShared<dyn ExecutorSharedFactory>,
  message_dispatcher_shared_factory: ArcShared<dyn MessageDispatcherSharedFactory>,
  shared_message_queue_factory: ArcShared<dyn SharedMessageQueueFactory>,
  actor_ref_sender_shared_factory: ArcShared<dyn ActorRefSenderSharedFactory>,
  actor_shared_lock_factory: ArcShared<dyn ActorSharedLockFactory>,
  actor_cell_state_shared_factory: ArcShared<dyn ActorCellStateSharedFactory>,
  receive_timeout_state_shared_factory: ArcShared<dyn ReceiveTimeoutStateSharedFactory>,
  message_invoker_shared_factory: ArcShared<dyn MessageInvokerSharedFactory>,
  actor_future_shared_factory: ArcShared<dyn ActorFutureSharedFactory<AskResult>>,
  tick_driver_control_shared_factory: ArcShared<dyn TickDriverControlSharedFactory>,
  local_actor_ref_provider_handle_shared_factory:
    ArcShared<dyn ActorRefProviderHandleSharedFactory<LocalActorRefProvider>>,
  event_stream_shared_factory: ArcShared<dyn EventStreamSharedFactory>,
  event_stream_subscriber_shared_factory: ArcShared<dyn EventStreamSubscriberSharedFactory>,
  mailbox_shared_set_factory: ArcShared<dyn MailboxSharedSetFactory>,
  context_pipe_waker_handle_shared_factory: ArcShared<dyn ContextPipeWakerHandleSharedFactory>,
  dispatchers: Dispatchers,
  mailboxes: Mailboxes,
  start_time: Option<Duration>,
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
    P: ExecutorSharedFactory
      + MessageDispatcherSharedFactory
      + SharedMessageQueueFactory
      + ActorRefSenderSharedFactory
      + ActorSharedLockFactory
      + ActorCellStateSharedFactory
      + ReceiveTimeoutStateSharedFactory
      + MessageInvokerSharedFactory
      + ActorFutureSharedFactory<AskResult>
      + TickDriverControlSharedFactory
      + ActorRefProviderHandleSharedFactory<LocalActorRefProvider>
      + EventStreamSharedFactory
      + EventStreamSubscriberSharedFactory
      + MailboxSharedSetFactory
      + ContextPipeWakerHandleSharedFactory
      + 'static, {
    let provider = ArcShared::new(provider);
    self.executor_shared_factory = provider.clone();
    self.message_dispatcher_shared_factory = provider.clone();
    self.shared_message_queue_factory = provider.clone();
    self.actor_ref_sender_shared_factory = provider.clone();
    self.actor_shared_lock_factory = provider.clone();
    self.actor_cell_state_shared_factory = provider.clone();
    self.receive_timeout_state_shared_factory = provider.clone();
    self.message_invoker_shared_factory = provider.clone();
    self.actor_future_shared_factory = provider.clone();
    self.tick_driver_control_shared_factory = provider.clone();
    self.local_actor_ref_provider_handle_shared_factory = provider.clone();
    self.event_stream_shared_factory = provider.clone();
    self.event_stream_subscriber_shared_factory = provider.clone();
    self.mailbox_shared_set_factory = provider.clone();
    self.context_pipe_waker_handle_shared_factory = provider;
    self
      .dispatchers
      .replace_default_inline_with_factories(&self.message_dispatcher_shared_factory, &self.executor_shared_factory);
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

  /// Returns the executor shared factory.
  #[must_use]
  pub const fn executor_shared_factory(&self) -> &ArcShared<dyn ExecutorSharedFactory> {
    &self.executor_shared_factory
  }

  /// Returns the message-dispatcher shared factory.
  #[must_use]
  pub const fn message_dispatcher_shared_factory(&self) -> &ArcShared<dyn MessageDispatcherSharedFactory> {
    &self.message_dispatcher_shared_factory
  }

  /// Returns the shared-message-queue factory.
  #[must_use]
  pub const fn shared_message_queue_factory(&self) -> &ArcShared<dyn SharedMessageQueueFactory> {
    &self.shared_message_queue_factory
  }

  /// Returns the actor-ref sender shared factory.
  #[must_use]
  pub const fn actor_ref_sender_shared_factory(&self) -> &ArcShared<dyn ActorRefSenderSharedFactory> {
    &self.actor_ref_sender_shared_factory
  }

  /// Returns the actor shared-lock factory.
  #[must_use]
  pub const fn actor_shared_lock_factory(&self) -> &ArcShared<dyn ActorSharedLockFactory> {
    &self.actor_shared_lock_factory
  }

  /// Returns the actor-cell-state shared factory.
  #[must_use]
  pub const fn actor_cell_state_shared_factory(&self) -> &ArcShared<dyn ActorCellStateSharedFactory> {
    &self.actor_cell_state_shared_factory
  }

  /// Returns the receive-timeout-state shared factory.
  #[must_use]
  pub const fn receive_timeout_state_shared_factory(&self) -> &ArcShared<dyn ReceiveTimeoutStateSharedFactory> {
    &self.receive_timeout_state_shared_factory
  }

  /// Returns the message-invoker shared factory.
  #[must_use]
  pub const fn message_invoker_shared_factory(&self) -> &ArcShared<dyn MessageInvokerSharedFactory> {
    &self.message_invoker_shared_factory
  }

  /// Returns the actor-future shared factory used by ask flows.
  #[must_use]
  pub const fn actor_future_shared_factory(&self) -> &ArcShared<dyn ActorFutureSharedFactory<AskResult>> {
    &self.actor_future_shared_factory
  }

  /// Returns the tick-driver-control shared factory.
  #[must_use]
  pub const fn tick_driver_control_shared_factory(&self) -> &ArcShared<dyn TickDriverControlSharedFactory> {
    &self.tick_driver_control_shared_factory
  }

  /// Returns the local actor-ref-provider handle shared factory.
  #[must_use]
  pub const fn local_actor_ref_provider_handle_shared_factory(
    &self,
  ) -> &ArcShared<dyn ActorRefProviderHandleSharedFactory<LocalActorRefProvider>> {
    &self.local_actor_ref_provider_handle_shared_factory
  }

  /// Returns the event-stream shared factory.
  #[must_use]
  pub const fn event_stream_shared_factory(&self) -> &ArcShared<dyn EventStreamSharedFactory> {
    &self.event_stream_shared_factory
  }

  /// Returns the event-stream-subscriber shared factory.
  #[must_use]
  pub const fn event_stream_subscriber_shared_factory(&self) -> &ArcShared<dyn EventStreamSubscriberSharedFactory> {
    &self.event_stream_subscriber_shared_factory
  }

  /// Returns the mailbox shared-set factory.
  #[must_use]
  pub const fn mailbox_shared_set_factory(&self) -> &ArcShared<dyn MailboxSharedSetFactory> {
    &self.mailbox_shared_set_factory
  }

  /// Returns the context-pipe-waker-handle shared factory.
  #[must_use]
  pub const fn context_pipe_waker_handle_shared_factory(&self) -> &ArcShared<dyn ContextPipeWakerHandleSharedFactory> {
    &self.context_pipe_waker_handle_shared_factory
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
    let shared_factory = ArcShared::new(BuiltinSpinSharedFactory::new());
    let mut dispatchers = Dispatchers::new();
    let message_dispatcher_shared_factory: ArcShared<dyn MessageDispatcherSharedFactory> = shared_factory.clone();
    let executor_shared_factory: ArcShared<dyn ExecutorSharedFactory> = shared_factory.clone();
    dispatchers.ensure_default_inline(&message_dispatcher_shared_factory, &executor_shared_factory);
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
      executor_shared_factory,
      message_dispatcher_shared_factory,
      shared_message_queue_factory: shared_factory.clone(),
      actor_ref_sender_shared_factory: shared_factory.clone(),
      actor_shared_lock_factory: shared_factory.clone(),
      actor_cell_state_shared_factory: shared_factory.clone(),
      receive_timeout_state_shared_factory: shared_factory.clone(),
      message_invoker_shared_factory: shared_factory.clone(),
      actor_future_shared_factory: shared_factory.clone(),
      tick_driver_control_shared_factory: shared_factory.clone(),
      local_actor_ref_provider_handle_shared_factory: shared_factory.clone(),
      event_stream_shared_factory: shared_factory.clone(),
      event_stream_subscriber_shared_factory: shared_factory.clone(),
      mailbox_shared_set_factory: shared_factory.clone(),
      context_pipe_waker_handle_shared_factory: shared_factory,
      dispatchers,
      mailboxes,
      start_time: None,
    }
  }
}
