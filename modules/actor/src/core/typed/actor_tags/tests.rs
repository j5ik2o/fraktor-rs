use crate::core::typed::{ActorTags, TypedProps};

#[test]
fn actor_tags_expose_distinct_tags() {
  let actor_tags = ActorTags::new(["frontend", "frontend", "edge"]);

  assert!(actor_tags.tags().contains("frontend"));
  assert!(actor_tags.tags().contains("edge"));
  assert_eq!(actor_tags.tags().len(), 2);
}

#[test]
fn actor_tags_apply_to_typed_props_replaces_tags_without_mutating_source_props() {
  let props = TypedProps::<u32>::empty().with_tag("existing");
  let configured = ActorTags::new(["frontend", "edge"]).apply_to(props.clone());
  let expected = TypedProps::<u32>::empty().with_tags(["frontend", "edge"]);

  assert!(props.to_untyped().tags().contains("existing"));
  assert_eq!(configured.to_untyped().tags(), expected.to_untyped().tags());
  assert!(!configured.to_untyped().tags().contains("existing"));
}
