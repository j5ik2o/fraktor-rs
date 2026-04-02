use core::num::NonZeroUsize;

use crate::core::typed::mailbox_selector::MailboxSelector;

#[test]
fn should_create_bounded() {
  let cap = NonZeroUsize::new(100).unwrap();
  let selector = MailboxSelector::bounded(cap);
  assert_eq!(selector, MailboxSelector::Bounded(cap));
}

#[test]
fn should_create_from_config() {
  let selector = MailboxSelector::from_config("priority-mailbox");
  assert_eq!(selector, MailboxSelector::FromConfig("priority-mailbox".into()));
}

#[test]
fn default_variant_matches_pekko_default_mailbox_contract() {
  let produced_selector = MailboxSelector::default();
  assert_eq!(produced_selector, MailboxSelector::Default);
}
