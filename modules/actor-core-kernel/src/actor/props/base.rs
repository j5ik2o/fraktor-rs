#[cfg(test)]
#[path = "base_test.rs"]
mod tests;

use alloc::{boxed::Box, collections::BTreeSet, string::String, vec::Vec};

use fraktor_utils_core_rs::collections::queue::capabilities::QueueCapabilityRegistry;

use super::{
  deployable_props_metadata::DeployablePropsMetadata, factory::ActorFactory, factory_shared::ActorFactoryShared,
  mailbox_config::MailboxConfig, mailbox_requirement::MailboxRequirement,
};
use crate::{actor::Actor, dispatch::mailbox::MailboxPolicy};

/// Immutable configuration describing how to construct an actor.
pub struct Props {
  factory: Option<ActorFactoryShared>,
  name: Option<String>,
  tags: BTreeSet<String>,
  mailbox_config: MailboxConfig,
  mailbox_id: Option<String>,
  middleware: Vec<String>,
  dispatcher_id: Option<String>,
  dispatcher_same_as_parent: bool,
  deployable_metadata: Option<DeployablePropsMetadata>,
}

impl Props {
  /// Creates new props from the provided factory.
  #[must_use]
  pub fn new(factory: Box<dyn ActorFactory>) -> Self {
    Self {
      factory: Some(ActorFactoryShared::new(factory)),
      name: None,
      tags: BTreeSet::new(),
      mailbox_config: MailboxConfig::default(),
      mailbox_id: None,
      middleware: Vec::new(),
      dispatcher_id: None,
      dispatcher_same_as_parent: false,
      deployable_metadata: None,
    }
  }

  /// Creates props without an actor factory.
  ///
  /// These props are only valid as a configuration builder and must not reach
  /// actor spawn without a factory being installed first.
  #[doc(hidden)]
  #[must_use]
  pub fn empty() -> Self {
    Self {
      factory: None,
      name: None,
      tags: BTreeSet::new(),
      mailbox_config: MailboxConfig::default(),
      mailbox_id: None,
      middleware: Vec::new(),
      dispatcher_id: None,
      dispatcher_same_as_parent: false,
      deployable_metadata: None,
    }
  }

  /// Convenience helper to build props from a closure.
  #[must_use]
  pub fn from_fn<F, A>(factory: F) -> Self
  where
    F: FnMut() -> A + Send + Sync + 'static,
    A: Actor + 'static, {
    Self::new(Box::new(factory))
  }

  /// Replaces the actor factory while preserving every other props setting.
  #[doc(hidden)]
  #[must_use]
  pub fn with_factory(mut self, factory: Box<dyn ActorFactory>) -> Self {
    self.factory = Some(ActorFactoryShared::new(factory));
    self
  }

  /// Returns the actor factory, if configured.
  #[must_use]
  pub const fn factory(&self) -> Option<&ActorFactoryShared> {
    self.factory.as_ref()
  }

  /// Returns the remote deployment metadata, if configured.
  #[must_use]
  pub const fn deployable_metadata(&self) -> Option<&DeployablePropsMetadata> {
    self.deployable_metadata.as_ref()
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

  /// Returns true when the dispatcher should be inherited from the parent actor.
  #[must_use]
  #[doc(hidden)]
  pub const fn dispatcher_same_as_parent(&self) -> bool {
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

  /// Marks these props as remotely deployable with wire-safe factory metadata.
  #[must_use]
  pub fn with_deployable_metadata(mut self, metadata: DeployablePropsMetadata) -> Self {
    self.deployable_metadata = Some(metadata);
    self
  }

  /// Clears remote deployment metadata.
  #[must_use]
  pub fn without_deployable_metadata(mut self) -> Self {
    self.deployable_metadata = None;
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

  /// Overrides the dispatcher by identifier.
  #[must_use]
  pub fn with_dispatcher_id(mut self, id: impl Into<String>) -> Self {
    self.dispatcher_id = Some(id.into());
    self.dispatcher_same_as_parent = false;
    self
  }

  /// Uses the same dispatcher configuration as the parent actor.
  #[must_use]
  pub fn with_dispatcher_same_as_parent(mut self) -> Self {
    self.dispatcher_id = None;
    self.dispatcher_same_as_parent = true;
    self
  }

  /// Overrides the mailbox requirements while preserving existing policy/configuration.
  #[must_use]
  pub fn with_mailbox_requirement(mut self, mailbox_requirement: MailboxRequirement) -> Self {
    self.mailbox_config = self.mailbox_config.with_requirement(mailbox_requirement);
    self
  }

  /// Configures the actor to use a deque-capable mailbox required by stash replay.
  #[must_use]
  pub fn with_stash_mailbox(self) -> Self {
    self.with_mailbox_requirement(MailboxRequirement::for_stash())
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
}

impl Clone for Props {
  fn clone(&self) -> Self {
    Self {
      factory: self.factory.clone(),
      name: self.name.clone(),
      tags: self.tags.clone(),
      mailbox_config: self.mailbox_config.clone(),
      mailbox_id: self.mailbox_id.clone(),
      middleware: self.middleware.clone(),
      dispatcher_id: self.dispatcher_id.clone(),
      dispatcher_same_as_parent: self.dispatcher_same_as_parent,
      deployable_metadata: self.deployable_metadata.clone(),
    }
  }
}
