//! Mailbox selection request.

use alloc::string::String;

use crate::actor::props::MailboxRequirement;

/// Inputs used to select a mailbox factory with Pekko-style precedence.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailboxSelection {
  explicit_mailbox_id:    Option<String>,
  dispatcher_mailbox_id:  Option<String>,
  actor_requirement:      MailboxRequirement,
  dispatcher_requirement: MailboxRequirement,
}

impl MailboxSelection {
  /// Creates an empty selection request.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      explicit_mailbox_id:    None,
      dispatcher_mailbox_id:  None,
      actor_requirement:      MailboxRequirement::none(),
      dispatcher_requirement: MailboxRequirement::none(),
    }
  }

  /// Returns the explicit mailbox id, if one was supplied.
  #[must_use]
  pub fn explicit_mailbox_id(&self) -> Option<&str> {
    self.explicit_mailbox_id.as_deref()
  }

  /// Returns the dispatcher mailbox id, if one was supplied.
  #[must_use]
  pub fn dispatcher_mailbox_id(&self) -> Option<&str> {
    self.dispatcher_mailbox_id.as_deref()
  }

  /// Returns the actor-side mailbox requirement.
  #[must_use]
  pub const fn actor_requirement(&self) -> MailboxRequirement {
    self.actor_requirement
  }

  /// Returns the dispatcher-side mailbox requirement.
  #[must_use]
  pub const fn dispatcher_requirement(&self) -> MailboxRequirement {
    self.dispatcher_requirement
  }

  /// Sets the explicit mailbox id.
  #[must_use]
  pub fn with_explicit_mailbox_id(mut self, id: impl Into<String>) -> Self {
    self.explicit_mailbox_id = Some(id.into());
    self
  }

  /// Sets the dispatcher mailbox id.
  #[must_use]
  pub fn with_dispatcher_mailbox_id(mut self, id: impl Into<String>) -> Self {
    self.dispatcher_mailbox_id = Some(id.into());
    self
  }

  /// Sets the actor-side mailbox requirement.
  #[must_use]
  pub const fn with_actor_requirement(mut self, requirement: MailboxRequirement) -> Self {
    self.actor_requirement = requirement;
    self
  }

  /// Sets the dispatcher-side mailbox requirement.
  #[must_use]
  pub const fn with_dispatcher_requirement(mut self, requirement: MailboxRequirement) -> Self {
    self.dispatcher_requirement = requirement;
    self
  }
}
