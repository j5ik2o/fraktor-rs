//! Dispatcher and mailbox state owned by SystemState.

#[cfg(test)]
#[path = "dispatch_mailbox_registry_test.rs"]
mod tests;

use crate::{
  dispatch::{dispatcher::Dispatchers, mailbox::Mailboxes},
  system::shared_factory::MailboxSharedSet,
};

/// Owns dispatcher and mailbox registries.
pub(crate) struct DispatchMailboxRegistry {
  pub(crate) dispatchers:        Dispatchers,
  pub(crate) mailboxes:          Mailboxes,
  pub(crate) mailbox_shared_set: MailboxSharedSet,
}

impl DispatchMailboxRegistry {
  pub(crate) const fn new(
    dispatchers: Dispatchers,
    mailboxes: Mailboxes,
    mailbox_shared_set: MailboxSharedSet,
  ) -> Self {
    Self { dispatchers, mailboxes, mailbox_shared_set }
  }
}
