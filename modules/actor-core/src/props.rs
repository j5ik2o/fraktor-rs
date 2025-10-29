//! Actor properties configuration.

mod mailbox_capacity;
mod mailbox_config;
mod supervisor_options;

use alloc::boxed::Box;

pub use mailbox_capacity::MailboxCapacity;
pub use mailbox_config::MailboxConfig;
pub use supervisor_options::SupervisorOptions;

use crate::{actor::Actor, mailbox_policy::MailboxPolicy};

/// Type alias for actor factory functions.
pub type ActorFactory = fn() -> Box<dyn Actor>;

/// Configuration data used when creating actors.
#[derive(Debug, Clone, Copy)]
pub struct Props {
  factory:    ActorFactory,
  mailbox:    MailboxConfig,
  supervisor: SupervisorOptions,
  throughput: u32,
  policy:     MailboxPolicy,
}

impl Props {
  /// Creates properties with the provided factory and default configuration.
  #[must_use]
  pub fn new(factory: ActorFactory) -> Self {
    Self {
      factory,
      mailbox: MailboxConfig::default(),
      supervisor: SupervisorOptions::default(),
      throughput: 300,
      policy: MailboxPolicy::Default,
    }
  }

  /// Returns the actor factory.
  #[must_use]
  pub const fn factory(&self) -> ActorFactory {
    self.factory
  }

  /// Returns the mailbox configuration.
  #[must_use]
  pub const fn mailbox(&self) -> &MailboxConfig {
    &self.mailbox
  }

  /// Updates the mailbox configuration.
  #[must_use]
  pub const fn with_mailbox(mut self, mailbox: MailboxConfig) -> Self {
    self.mailbox = mailbox;
    self
  }

  /// Returns the supervisor options.
  #[must_use]
  pub const fn supervisor(&self) -> &SupervisorOptions {
    &self.supervisor
  }

  /// Updates the supervisor options.
  #[must_use]
  pub const fn with_supervisor(mut self, supervisor: SupervisorOptions) -> Self {
    self.supervisor = supervisor;
    self
  }

  /// Returns the throughput limit per scheduling turn.
  #[must_use]
  pub const fn throughput(&self) -> u32 {
    self.throughput
  }

  /// Sets the throughput limit per scheduling turn.
  #[must_use]
  pub const fn with_throughput(mut self, throughput: u32) -> Self {
    self.throughput = throughput;
    self
  }

  /// Returns the mailbox overflow policy.
  #[must_use]
  pub const fn policy(&self) -> MailboxPolicy {
    self.policy
  }

  /// Updates the mailbox overflow policy.
  #[must_use]
  pub const fn with_policy(mut self, policy: MailboxPolicy) -> Self {
    self.policy = policy;
    self
  }
}
