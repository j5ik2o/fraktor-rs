//! Actor factory configuration and runtime policies.

mod mailbox_config;
mod supervisor_options;

use cellactor_utils_core_rs::ArcShared;
pub use mailbox_config::MailboxConfig;
pub use supervisor_options::SupervisorOptions;

use crate::{actor::Actor, actor_context::ActorContext};

type ActorFactory = ArcShared<
  dyn for<'a> Fn(&'a ActorContext<'a>) -> ArcShared<dyn Actor + Send + Sync + 'static> + Send + Sync + 'static,
>;

/// Configuration object describing how to create actors and wire their runtime policies.
pub struct Props {
  factory:    ActorFactory,
  mailbox:    MailboxConfig,
  supervisor: SupervisorOptions,
}

impl Props {
  /// Creates props from the provided actor factory.
  #[must_use]
  pub fn new<F>(factory: F) -> Self
  where
    F: for<'a> Fn(&'a ActorContext<'a>) -> ArcShared<dyn Actor + Send + Sync + 'static> + Send + Sync + 'static, {
    Self {
      factory:    ArcShared::new(factory),
      mailbox:    MailboxConfig::default(),
      supervisor: SupervisorOptions::default(),
    }
  }

  /// Overrides the mailbox configuration.
  #[must_use]
  pub fn with_mailbox(mut self, mailbox: MailboxConfig) -> Self {
    self.mailbox = mailbox;
    self
  }

  /// Overrides the supervisor configuration.
  #[must_use]
  pub fn with_supervisor(mut self, supervisor: SupervisorOptions) -> Self {
    self.supervisor = supervisor;
    self
  }

  /// Returns the mailbox configuration.
  #[must_use]
  pub const fn mailbox(&self) -> &MailboxConfig {
    &self.mailbox
  }

  /// Returns the supervisor configuration.
  #[must_use]
  pub const fn supervisor(&self) -> &SupervisorOptions {
    &self.supervisor
  }

  /// Invokes the actor factory to create a new actor instance.
  #[must_use]
  pub fn create_actor(&self, ctx: &ActorContext<'_>) -> ArcShared<dyn Actor + Send + Sync + 'static> {
    (self.factory)(ctx)
  }
}
