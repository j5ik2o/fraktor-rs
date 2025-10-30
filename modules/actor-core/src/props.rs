//! Actor construction descriptors.

use alloc::{string::String, vec::Vec};
use core::num::NonZeroUsize;

use cellactor_utils_core_rs::sync::ArcShared;

use crate::{actor::Actor, mailbox_policy::MailboxPolicy, supervisor_strategy::SupervisorStrategy};

/// Trait implemented by actor factories stored inside [`Props`].
pub trait ActorFactory: Send + Sync {
  /// Creates a new actor instance wrapped in [`ArcShared`].
  fn create(&self) -> ArcShared<dyn Actor + Send + Sync>;
}

impl<F, A> ActorFactory for F
where
  F: Fn() -> A + Send + Sync + 'static,
  A: Actor + Sync + 'static,
{
  fn create(&self) -> ArcShared<dyn Actor + Send + Sync> {
    let actor = (self)();
    ArcShared::new(actor)
  }
}

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
  pub fn with_mailbox(mut self, config: MailboxConfig) -> Self {
    self.mailbox = config;
    self
  }

  /// Updates the supervisor options.
  #[must_use]
  pub fn with_supervisor(mut self, supervisor: SupervisorOptions) -> Self {
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

/// Mailbox configuration derived from the props builder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxConfig {
  policy:         MailboxPolicy,
  warn_threshold: Option<NonZeroUsize>,
}

impl MailboxConfig {
  /// Creates a new mailbox configuration.
  #[must_use]
  pub const fn new(policy: MailboxPolicy) -> Self {
    Self { policy, warn_threshold: None }
  }

  /// Returns the mailbox policy.
  #[must_use]
  pub const fn policy(&self) -> MailboxPolicy {
    self.policy
  }

  /// Returns the warning threshold.
  #[must_use]
  pub const fn warn_threshold(&self) -> Option<NonZeroUsize> {
    self.warn_threshold
  }

  /// Updates the warning threshold.
  #[must_use]
  pub const fn with_warn_threshold(mut self, threshold: Option<NonZeroUsize>) -> Self {
    self.warn_threshold = threshold;
    self
  }
}

impl Default for MailboxConfig {
  fn default() -> Self {
    MailboxConfig::new(MailboxPolicy::unbounded(None))
  }
}

/// Supervisor configuration attached to props.
#[derive(Clone, Copy, Debug)]
pub struct SupervisorOptions {
  strategy: SupervisorStrategy,
}

impl SupervisorOptions {
  /// Creates supervisor options.
  #[must_use]
  pub const fn new(strategy: SupervisorStrategy) -> Self {
    Self { strategy }
  }

  /// Returns the configured strategy.
  #[must_use]
  pub const fn strategy(&self) -> &SupervisorStrategy {
    &self.strategy
  }
}

impl Default for SupervisorOptions {
  fn default() -> Self {
    const DEFAULT_WITHIN: core::time::Duration = core::time::Duration::from_secs(1);
    fn decide(_: &crate::actor_error::ActorError) -> crate::supervisor_strategy::SupervisorDirective {
      crate::supervisor_strategy::SupervisorDirective::Restart
    }
    Self::new(SupervisorStrategy::new(
      crate::supervisor_strategy::SupervisorStrategyKind::OneForOne,
      10,
      DEFAULT_WITHIN,
      decide,
    ))
  }
}
