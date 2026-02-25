use alloc::collections::BTreeSet;
use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::tagged::Tagged;

#[test]
fn tagged_holds_payload_and_tags() {
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(9_i32);
  let mut tags = BTreeSet::new();
  tags.insert("blue".into());
  tags.insert("metrics".into());
  let tagged = Tagged::new(payload, tags);

  assert_eq!(tagged.downcast_ref::<i32>(), Some(&9_i32));
  assert!(tagged.contains_tag("blue"));
  assert!(tagged.contains_tag("metrics"));
  assert!(!tagged.contains_tag("missing"));
}

#[test]
fn tagged_with_tags_deduplicates_input_tags() {
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new("event");
  let tagged = Tagged::with_tags(payload, ["region-ap", "region-ap", "source-a"]);

  assert_eq!(tagged.tags().len(), 2);
  assert!(tagged.contains_tag("region-ap"));
  assert!(tagged.contains_tag("source-a"));
}
