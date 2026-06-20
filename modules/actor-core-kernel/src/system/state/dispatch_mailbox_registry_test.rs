use super::DispatchMailboxRegistry;
use crate::{
  dispatch::{dispatcher::Dispatchers, mailbox::Mailboxes},
  system::shared_factory::MailboxSharedSet,
};

#[test]
fn dispatch_mailbox_registry_preserves_supplied_handles() {
  let mut dispatchers = Dispatchers::new();
  dispatchers.ensure_default_inline();
  let mut mailboxes = Mailboxes::new();
  mailboxes.ensure_default();

  let registry = DispatchMailboxRegistry::new(dispatchers, mailboxes, MailboxSharedSet::builtin());

  assert!(registry.dispatchers.resolve("fraktor.actor.internal-dispatcher").is_ok());
  assert!(registry.mailboxes.resolve("fraktor.actor.default-mailbox").is_ok());
}
