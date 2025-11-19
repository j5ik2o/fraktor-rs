use alloc::{string::String, vec::Vec};

use fraktor_utils_rs::core::{
  collections::queue::capabilities::QueueCapabilityRegistry,
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::ArcShared,
};

use super::{
  dispatcher_config::DispatcherConfigGeneric, factory::ActorFactory, mailbox_config::MailboxConfig,
  mailbox_requirement::MailboxRequirement,
};
use crate::core::{actor_prim::Actor, mailbox::MailboxPolicy};

/// Immutable configuration describing how to construct an actor.
pub struct PropsGeneric<TB: RuntimeToolbox + 'static> {
  factory:           ArcShared<dyn ActorFactory<TB>>,
  name:              Option<String>,
  mailbox:           MailboxConfig,
  mailbox_id:        Option<String>,
  middleware:        Vec<String>,
  dispatcher:        DispatcherConfigGeneric<TB>,
  dispatcher_id:     Option<String>,
  dispatcher_custom: bool,
}

/// Type alias for [PropsGeneric] with the default [NoStdToolbox].
pub type Props = PropsGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> PropsGeneric<TB> {
  /// Creates new props from the provided factory.
  #[must_use]
  pub fn new(factory: ArcShared<dyn ActorFactory<TB>>) -> Self {
    Self {
      factory,
      name: None,
      mailbox: MailboxConfig::default(),
      mailbox_id: None,
      middleware: Vec::new(),
      dispatcher: DispatcherConfigGeneric::default(),
      dispatcher_id: None,
      dispatcher_custom: false,
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

  /// Returns the configured mailbox identifier, if any.
  #[must_use]
  pub fn mailbox_id(&self) -> Option<&str> {
    self.mailbox_id.as_deref()
  }

  /// Returns the mailbox policy.
  #[must_use]
  pub const fn mailbox_policy(&self) -> MailboxPolicy {
    self.mailbox.policy()
  }

  /// Returns the mailbox requirements.
  #[must_use]
  pub const fn mailbox_requirement(&self) -> MailboxRequirement {
    self.mailbox.requirement()
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

  pub(crate) const fn has_custom_dispatcher(&self) -> bool {
    self.dispatcher_custom
  }

  /// Returns the configured dispatcher identifier, if any.
  #[must_use]
  pub fn dispatcher_id(&self) -> Option<&str> {
    self.dispatcher_id.as_deref()
  }

  /// Updates the mailbox configuration.
  #[must_use]
  pub fn with_mailbox(mut self, config: MailboxConfig) -> Self {
    self.mailbox = config;
    self.mailbox_id = None;
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
    self.dispatcher_id = None;
    self.dispatcher_custom = true;
    self
  }

  /// Overrides the dispatcher by identifier.
  #[must_use]
  pub fn with_dispatcher_id(mut self, id: impl Into<String>) -> Self {
    self.dispatcher_id = Some(id.into());
    self.dispatcher_custom = false;
    self
  }

  /// Overrides the mailbox requirements while preserving existing policy/configuration.
  #[must_use]
  pub const fn with_mailbox_requirement(mut self, requirement: MailboxRequirement) -> Self {
    self.mailbox = self.mailbox.with_requirement(requirement);
    self
  }

  /// Overrides the mailbox via identifier.
  #[must_use]
  pub fn with_mailbox_id(mut self, id: impl Into<String>) -> Self {
    self.mailbox_id = Some(id.into());
    self
  }

  /// Overrides the mailbox capability registry (testing helper).
  #[must_use]
  pub fn with_mailbox_capabilities(mut self, registry: QueueCapabilityRegistry) -> Self {
    self.mailbox = self.mailbox.with_capabilities(registry);
    self.mailbox_id = None;
    self
  }

  pub(crate) fn with_resolved_dispatcher(mut self, dispatcher: DispatcherConfigGeneric<TB>) -> Self {
    self.dispatcher = dispatcher;
    self.dispatcher_id = None;
    self.dispatcher_custom = true;
    self
  }

  pub(crate) fn with_resolved_mailbox(mut self, mailbox: MailboxConfig) -> Self {
    self.mailbox = mailbox;
    self.mailbox_id = None;
    self
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for PropsGeneric<TB> {
  fn clone(&self) -> Self {
    Self {
      factory:           self.factory.clone(),
      name:              self.name.clone(),
      mailbox:           self.mailbox,
      mailbox_id:        self.mailbox_id.clone(),
      middleware:        self.middleware.clone(),
      dispatcher:        self.dispatcher.clone(),
      dispatcher_id:     self.dispatcher_id.clone(),
      dispatcher_custom: self.dispatcher_custom,
    }
  }
}
