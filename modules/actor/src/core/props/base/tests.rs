use alloc::collections::BTreeSet;

use crate::core::{
  actor::{Actor, ActorContext},
  error::ActorError,
  messaging::AnyMessageView,
  props::Props,
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
