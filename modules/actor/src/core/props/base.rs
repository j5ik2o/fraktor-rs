#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::BTreeSet, string::String, vec::Vec};

use fraktor_utils_rs::core::collections::queue::capabilities::QueueCapabilityRegistry;

use super::{
  factory::ActorFactory, factory_shared::ActorFactoryShared, mailbox_config::MailboxConfig,
  mailbox_requirement::MailboxRequirement,
};
use crate::core::{
  actor::Actor,
  dispatch::{dispatcher::DispatcherConfig, mailbox::MailboxPolicy},
};

/// Immutable configuration describing how to construct an actor.
pub struct Props {
  factory: ActorFactoryShared,
  name: Option<String>,
  tags: BTreeSet<String>,
  mailbox_config: MailboxConfig,
  mailbox_id: Option<String>,
  middleware: Vec<String>,
  dispatcher_config: DispatcherConfig,
  dispatcher_id: Option<String>,
  dispatcher_custom: bool,
  dispatcher_same_as_parent: bool,
}

impl Props {
  /// Creates new props from the provided factory.
  #[must_use]
  pub fn new(factory: Box<dyn ActorFactory>) -> Self {
    Self {
      factory: ActorFactoryShared::new(factory),
      name: None,
      tags: BTreeSet::new(),
      mailbox_config: MailboxConfig::default(),
      mailbox_id: None,
      middleware: Vec::new(),
      dispatcher_config: DispatcherConfig::default(),
      dispatcher_id: None,
      dispatcher_custom: false,
      dispatcher_same_as_parent: false,
    }
  }

  /// Convenience helper to build props from a closure.
  #[must_use]
  pub fn from_fn<F, A>(factory: F) -> Self
  where
    F: FnMut() -> A + Send + Sync + 'static,
    A: Actor + Sync + 'static, {
    Self::new(Box::new(factory))
  }

  /// Returns the actor factory.
  #[must_use]
  pub const fn factory(&self) -> &ActorFactoryShared {
    &self.factory
  }

  /// Returns the configured actor name, if any.
  #[must_use]
  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  /// Returns the metadata tags associated with the actor.
  ///
  /// This mirrors Pekko's `ActorTags`. Tags are arbitrary string labels for
  /// observability, grouping, or routing purposes.
  #[must_use]
  pub const fn tags(&self) -> &BTreeSet<String> {
    &self.tags
  }

  /// Returns the mailbox configuration.
  #[must_use]
  pub const fn mailbox_config(&self) -> &MailboxConfig {
    &self.mailbox_config
  }

  /// Returns the configured mailbox identifier, if any.
  #[must_use]
  pub fn mailbox_id(&self) -> Option<&str> {
    self.mailbox_id.as_deref()
  }

  /// Returns the mailbox policy.
  #[must_use]
  pub const fn mailbox_policy(&self) -> MailboxPolicy {
    self.mailbox_config.policy()
  }

  /// Returns the mailbox requirements.
  #[must_use]
  pub const fn mailbox_requirement(&self) -> MailboxRequirement {
    self.mailbox_config.requirement()
  }

  /// Returns the registered middleware identifiers.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub fn middleware(&self) -> &[String] {
    &self.middleware
  }

  /// Returns the configured dispatcher settings.
  #[must_use]
  pub const fn dispatcher_config(&self) -> &DispatcherConfig {
    &self.dispatcher_config
  }

  pub(crate) const fn has_custom_dispatcher(&self) -> bool {
    self.dispatcher_custom
  }

  /// Returns true when the dispatcher should be inherited from the parent actor.
  #[must_use]
  pub(crate) const fn dispatcher_same_as_parent(&self) -> bool {
    self.dispatcher_same_as_parent
  }

  /// Returns the configured dispatcher identifier, if any.
  #[must_use]
  pub fn dispatcher_id(&self) -> Option<&str> {
    self.dispatcher_id.as_deref()
  }

  /// Updates the mailbox configuration.
  #[must_use]
  pub fn with_mailbox_config(mut self, mailbox_config: MailboxConfig) -> Self {
    self.mailbox_config = mailbox_config;
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

  /// Clears any previously assigned name, making this an anonymous actor.
  #[must_use]
  pub fn without_name(mut self) -> Self {
    self.name = None;
    self
  }

  /// Attaches metadata tags to the actor for observability and routing.
  ///
  /// This mirrors Pekko's `ActorTags`.
  #[must_use]
  pub fn with_tags<I, S>(mut self, tags: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>, {
    self.tags = tags.into_iter().map(Into::into).collect();
    self
  }

  /// Adds a single metadata tag to the actor.
  #[must_use]
  pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
    self.tags.insert(tag.into());
    self
  }

  /// Overrides the dispatcher configuration used when constructing actors.
  #[must_use]
  pub fn with_dispatcher_config(mut self, dispatcher_config: DispatcherConfig) -> Self {
    self.dispatcher_config = dispatcher_config;
    self.dispatcher_id = None;
    self.dispatcher_custom = true;
    self.dispatcher_same_as_parent = false;
    self
  }

  /// Overrides the dispatcher by identifier.
  #[must_use]
  pub fn with_dispatcher_id(mut self, id: impl Into<String>) -> Self {
    self.dispatcher_id = Some(id.into());
    self.dispatcher_custom = false;
    self.dispatcher_same_as_parent = false;
    self
  }

  /// Uses the same dispatcher configuration as the parent actor.
  #[must_use]
  pub fn with_dispatcher_same_as_parent(mut self) -> Self {
    self.dispatcher_id = None;
    self.dispatcher_custom = false;
    self.dispatcher_same_as_parent = true;
    self
  }

  /// Overrides the mailbox requirements while preserving existing policy/configuration.
  #[must_use]
  pub const fn with_mailbox_requirement(mut self, mailbox_requirement: MailboxRequirement) -> Self {
    self.mailbox_config = self.mailbox_config.with_requirement(mailbox_requirement);
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
  pub fn with_mailbox_capabilities(mut self, queue_capability_registry: QueueCapabilityRegistry) -> Self {
    self.mailbox_config = self.mailbox_config.with_capabilities(queue_capability_registry);
    self.mailbox_id = None;
    self
  }

  pub(crate) fn with_resolved_dispatcher_config(mut self, dispatcher_config: DispatcherConfig) -> Self {
    self.dispatcher_config = dispatcher_config;
    self.dispatcher_id = None;
    self.dispatcher_custom = true;
    self.dispatcher_same_as_parent = false;
    self
  }

  pub(crate) const fn with_resolved_mailbox_config(mut self, mailbox_config: MailboxConfig) -> Self {
    self.mailbox_config = mailbox_config;
    self
  }
}

impl Clone for Props {
  fn clone(&self) -> Self {
    Self {
      factory: self.factory.clone(),
      name: self.name.clone(),
      tags: self.tags.clone(),
      mailbox_config: self.mailbox_config,
      mailbox_id: self.mailbox_id.clone(),
      middleware: self.middleware.clone(),
      dispatcher_config: self.dispatcher_config.clone(),
      dispatcher_id: self.dispatcher_id.clone(),
      dispatcher_custom: self.dispatcher_custom,
      dispatcher_same_as_parent: self.dispatcher_same_as_parent,
    }
  }
}
