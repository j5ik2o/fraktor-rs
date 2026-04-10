use core::num::NonZeroUsize;

use super::TypedProps;
use crate::core::{kernel::actor::props::MailboxConfigError, typed::MailboxSelector};

#[test]
fn with_mailbox_from_config_sets_mailbox_id() {
  let props = TypedProps::<u32>::empty().with_mailbox_from_config("priority-mailbox");

  assert_eq!(props.to_untyped().mailbox_id(), Some("priority-mailbox"));
}

#[test]
fn with_mailbox_from_config_keeps_original_props_unchanged() {
  let props = TypedProps::<u32>::empty();
  let configured = props.clone().with_mailbox_from_config("priority-mailbox");

  assert_eq!(props.to_untyped().mailbox_id(), None);
  assert_eq!(configured.to_untyped().mailbox_id(), Some("priority-mailbox"));
}

#[test]
fn with_mailbox_from_config_matches_explicit_selector_path() {
  let via_shorthand = TypedProps::<u32>::empty().with_mailbox_from_config("priority-mailbox");
  let via_selector = TypedProps::<u32>::empty().with_mailbox_selector(MailboxSelector::from_config("priority-mailbox"));

  assert_eq!(via_shorthand.to_untyped().mailbox_id(), via_selector.to_untyped().mailbox_id());
}

#[test]
fn with_stash_mailbox_sets_stash_requirement() {
  let props = TypedProps::<u32>::empty().with_stash_mailbox();

  assert_eq!(
    props.to_untyped().mailbox_requirement(),
    crate::core::kernel::actor::props::MailboxRequirement::for_stash()
  );
}

#[test]
fn with_stash_mailbox_rejects_bounded_mailbox_config() {
  let props = TypedProps::<u32>::empty()
    .with_mailbox_selector(MailboxSelector::Bounded(NonZeroUsize::new(8).expect("non-zero")))
    .with_stash_mailbox();

  assert_eq!(props.to_untyped().mailbox_config().validate(), Err(MailboxConfigError::BoundedWithDeque));
}
