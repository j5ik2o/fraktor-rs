use alloc::{string::String, vec::Vec};

use cellactor_utils_core_rs::sync::ArcShared;

use super::{
  dispatcher_config::DispatcherConfigGeneric, factory::ActorFactory, mailbox_config::MailboxConfig,
  supervisor_options::SupervisorOptions,
};
use crate::{NoStdToolbox, RuntimeToolbox, actor_prim::Actor, mailbox::MailboxPolicy};

/// Immutable configuration describing how to construct an actor.
pub struct PropsGeneric<TB: RuntimeToolbox + 'static> {
  factory:    ArcShared<dyn ActorFactory<TB>>,
  name:       Option<String>,
  mailbox:    MailboxConfig,
  supervisor: SupervisorOptions,
  middleware: Vec<String>,
  dispatcher: DispatcherConfigGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> PropsGeneric<TB> {
  /// Creates new props from the provided factory.
  #[must_use]
  pub fn new(factory: ArcShared<dyn ActorFactory<TB>>) -> Self {
    Self {
      factory,
      name: None,
      mailbox: MailboxConfig::default(),
      supervisor: SupervisorOptions::default(),
      middleware: Vec::new(),
      dispatcher: DispatcherConfigGeneric::default(),
    }
  }

  /// Convenience helper to build props from a closure.
  #[must_use]
  pub fn from_fn<F, A>(factory: F) -> Self
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor<TB> + Sync + 'static, {
    Self::new(ArcShared::new(factory))
  }

  /// Returns the actor factory.
  #[must_use]
  pub fn factory(&self) -> &ArcShared<dyn ActorFactory<TB>> {
    &self.factory
  }

  /// Returns the configured actor name, if any.
  #[must_use]
  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  /// Returns the mailbox configuration.
  #[must_use]
  pub const fn mailbox(&self) -> &MailboxConfig {
    &self.mailbox
  }

  /// Returns the mailbox policy.
  #[must_use]
  pub const fn mailbox_policy(&self) -> MailboxPolicy {
    self.mailbox.policy()
  }

  /// Returns the supervisor options.
  #[must_use]
  pub const fn supervisor(&self) -> &SupervisorOptions {
    &self.supervisor
  }

  /// Returns the registered middleware identifiers.
  #[must_use]
  pub fn middleware(&self) -> &[String] {
    &self.middleware
  }

  /// Returns the configured dispatcher settings.
  #[must_use]
  pub const fn dispatcher(&self) -> &DispatcherConfigGeneric<TB> {
    &self.dispatcher
  }

  /// Updates the mailbox configuration.
  #[must_use]
  pub const fn with_mailbox(mut self, config: MailboxConfig) -> Self {
    self.mailbox = config;
    self
  }

  /// Updates the supervisor options.
  #[must_use]
  pub const fn with_supervisor(mut self, supervisor: SupervisorOptions) -> Self {
    self.supervisor = supervisor;
    self
  }

  /// Registers middleware identifiers used when constructing the message pipeline.
  #[must_use]
  pub fn with_middleware<I, S>(mut self, middleware: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>, {
    self.middleware = middleware.into_iter().map(Into::into).collect();
    self
  }

  /// Assigns a logical name to the actor for registry purposes.
  #[must_use]
  pub fn with_name(mut self, name: impl Into<String>) -> Self {
    self.name = Some(name.into());
    self
  }

  /// Overrides the dispatcher configuration used when constructing actors.
  #[must_use]
  pub fn with_dispatcher(mut self, dispatcher: DispatcherConfigGeneric<TB>) -> Self {
    self.dispatcher = dispatcher;
    self
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for PropsGeneric<TB> {
  fn clone(&self) -> Self {
    Self {
      factory:    self.factory.clone(),
      name:       self.name.clone(),
      mailbox:    self.mailbox,
      supervisor: self.supervisor,
      middleware: self.middleware.clone(),
      dispatcher: self.dispatcher.clone(),
    }
  }
}

/// Type alias for `PropsGeneric` with the default `NoStdToolbox`.
pub type Props = PropsGeneric<NoStdToolbox>;
