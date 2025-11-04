use core::num::NonZeroUsize;

use super::MailboxConfig;
use crate::mailbox::MailboxPolicy;

#[test]
fn mailbox_config_new() {
  let config = MailboxConfig::new(MailboxPolicy::unbounded(None));
  assert_eq!(config.policy(), MailboxPolicy::unbounded(None));
  assert_eq!(config.warn_threshold(), None);
}

#[test]
fn mailbox_config_policy() {
  let config = MailboxConfig::new(MailboxPolicy::unbounded(None));
  assert_eq!(config.policy(), MailboxPolicy::unbounded(None));
}

#[test]
fn mailbox_config_warn_threshold() {
  let config = MailboxConfig::new(MailboxPolicy::unbounded(None));
  assert_eq!(config.warn_threshold(), None);

  let threshold = NonZeroUsize::new(100).unwrap();
  let config_with_threshold = config.with_warn_threshold(Some(threshold));
  assert_eq!(config_with_threshold.warn_threshold(), Some(threshold));
}

#[test]
fn mailbox_config_with_warn_threshold() {
  let config = MailboxConfig::new(MailboxPolicy::unbounded(None));
  let threshold = NonZeroUsize::new(50).unwrap();
  let updated = config.with_warn_threshold(Some(threshold));
  assert_eq!(updated.warn_threshold(), Some(threshold));

  // ???????????
  assert_eq!(config.warn_threshold(), None);
}

#[test]
fn mailbox_config_default() {
  let config = MailboxConfig::default();
  assert_eq!(config.policy(), MailboxPolicy::unbounded(None));
  assert_eq!(config.warn_threshold(), None);
}

#[test]
fn mailbox_config_clone() {
  let config1 = MailboxConfig::new(MailboxPolicy::unbounded(None));
  let config2 = config1; // Copy trait???clone()??
  assert_eq!(config1, config2);
}

#[test]
fn mailbox_config_copy() {
  let config1 = MailboxConfig::new(MailboxPolicy::unbounded(None));
  let config2 = config1;
  assert_eq!(config1, config2);
}

#[test]
fn mailbox_config_debug() {
  // Debug?????????????????????????????
  let config = MailboxConfig::new(MailboxPolicy::unbounded(None));
  fn assert_debug<T: core::fmt::Debug>(_t: &T) {}
  assert_debug(&config);
}

#[test]
fn mailbox_config_partial_eq() {
  let config1 = MailboxConfig::new(MailboxPolicy::unbounded(None));
  let config2 = MailboxConfig::new(MailboxPolicy::unbounded(None));
  assert_eq!(config1, config2);
}

#[test]
fn mailbox_config_eq() {
  let config1 = MailboxConfig::new(MailboxPolicy::unbounded(None));
  let config2 = MailboxConfig::new(MailboxPolicy::unbounded(None));
  assert_eq!(config1, config2);
}
