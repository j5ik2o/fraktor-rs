use alloc::collections::BTreeSet;
use core::num::NonZeroUsize;

use crate::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::AnyMessageView,
    props::{MailboxConfig, Props},
  },
  dispatch::mailbox::{MailboxOverflowStrategy, MailboxPolicy},
};

struct TestActor;

impl Actor for TestActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _msg: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn tags_empty_by_default() {
  let props = Props::from_fn(|| TestActor);
  assert!(props.tags().is_empty());
}

#[test]
fn empty_props_do_not_have_factory() {
  let props = Props::empty();
  assert!(props.factory().is_none());
  assert!(props.tags().is_empty());
}

#[test]
fn with_tags_sets_tags() {
  let props = Props::from_fn(|| TestActor).with_tags(["foo", "bar"]);
  let mut expected = BTreeSet::new();
  expected.insert("bar".into());
  expected.insert("foo".into());
  assert_eq!(*props.tags(), expected);
}

#[test]
fn with_tag_adds_single_tag() {
  let props = Props::from_fn(|| TestActor).with_tag("alpha").with_tag("beta");
  assert!(props.tags().contains("alpha"));
  assert!(props.tags().contains("beta"));
  assert_eq!(props.tags().len(), 2);
}

#[test]
fn clone_preserves_tags() {
  let props = Props::from_fn(|| TestActor).with_tags(["a", "b"]);
  let cloned = props.clone();
  assert_eq!(*cloned.tags(), *props.tags());
}

#[test]
fn with_stash_mailbox_sets_stash_requirement() {
  let props = Props::from_fn(|| TestActor).with_stash_mailbox();

  assert_eq!(props.mailbox_requirement(), crate::core::kernel::actor::props::MailboxRequirement::for_stash());
}

#[test]
fn with_stash_mailbox_accepts_bounded_mailbox_config() {
  let props = Props::from_fn(|| TestActor)
    .with_mailbox_config(MailboxConfig::new(MailboxPolicy::bounded(
      NonZeroUsize::new(8).expect("non-zero"),
      MailboxOverflowStrategy::DropNewest,
      None,
    )))
    .with_stash_mailbox();

  assert_eq!(props.mailbox_config().validate(), Ok(()));
}
