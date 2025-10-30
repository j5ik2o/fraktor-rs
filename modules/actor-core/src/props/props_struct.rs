use alloc::{string::String, vec::Vec};

use cellactor_utils_core_rs::sync::ArcShared;

use super::{actor_factory::ActorFactory, mailbox_config::MailboxConfig, supervisor_options::SupervisorOptions};
use crate::actor::Actor;

/// Immutable configuration describing how to construct an actor.
pub struct Props {
  factory:    ArcShared<dyn ActorFactory>,
  mailbox:    MailboxConfig,
  supervisor: SupervisorOptions,
  middleware: Vec<String>,
}

impl Props {
  /// Creates new props from the provided factory.
  #[must_use]
  pub fn new(factory: ArcShared<dyn ActorFactory>) -> Self {
    Self {
      factory,
      mailbox: MailboxConfig::default(),
      supervisor: SupervisorOptions::default(),
      middleware: Vec::new(),
    }
  }

  /// Convenience helper to build props from a closure.
  #[must_use]
  pub fn from_fn<F, A>(factory: F) -> Self
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + Sync + 'static, {
    Self::new(ArcShared::new(factory))
  }

  /// Returns the actor factory.
  #[must_use]
  pub fn factory(&self) -> &ArcShared<dyn ActorFactory> {
    &self.factory
  }

  /// Returns the mailbox configuration.
  #[must_use]
  pub const fn mailbox(&self) -> &MailboxConfig {
    &self.mailbox
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
}
