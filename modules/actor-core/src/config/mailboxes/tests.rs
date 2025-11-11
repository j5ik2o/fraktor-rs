use super::*;
use crate::props::MailboxConfig;

#[test]
fn register_and_resolve_mailbox() {
  let registry = MailboxesGeneric::<NoStdToolbox>::new();
  registry.ensure_default();
  let config = MailboxConfig::default().with_warn_threshold(None);
  registry.register("custom", config).expect("register mailbox");
  assert!(registry.resolve("custom").is_ok());
}

#[test]
fn register_duplicate_mailbox_fails() {
  let registry = MailboxesGeneric::<NoStdToolbox>::new();
  registry.ensure_default();
  let config = MailboxConfig::default();
  registry.register("dup", config).expect("first register");
  assert!(matches!(registry.register("dup", MailboxConfig::default()), Err(ConfigError::MailboxDuplicate(_))));
}

#[test]
fn ensure_default_mailbox_is_available() {
  let registry = MailboxesGeneric::<NoStdToolbox>::new();
  registry.ensure_default();
  assert!(registry.resolve(DEFAULT_MAILBOX_ID).is_ok());
}
