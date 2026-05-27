use alloc::{collections::BTreeSet, string::String};

use crate::{PersistenceId, PublishedEvent};

#[test]
fn published_event_holds_public_event_stream_metadata() {
  let persistence_id = PersistenceId::of_unique_id("pid-published");
  let tags = BTreeSet::from([String::from("blue"), String::from("fast")]);
  let published = PublishedEvent::new(persistence_id.clone(), 15, String::from("event-1"), 900, tags.clone());
  assert_eq!(published.persistence_id(), &persistence_id);
  assert_eq!(published.sequence_nr(), 15);
  assert_eq!(published.event().as_str(), "event-1");
  assert_eq!(published.timestamp(), 900);
  assert_eq!(published.tags(), &tags);
}

#[test]
fn without_tags_returns_same_event_metadata_with_empty_tags() {
  let persistence_id = PersistenceId::of_unique_id("pid-published");
  let tags = BTreeSet::from([String::from("tagged")]);
  let published = PublishedEvent::new(persistence_id.clone(), 16, String::from("event-2"), 901, tags);
  let without_tags = published.without_tags();
  assert_eq!(without_tags.persistence_id(), &persistence_id);
  assert_eq!(without_tags.sequence_nr(), 16);
  assert_eq!(without_tags.event().as_str(), "event-2");
  assert_eq!(without_tags.timestamp(), 901);
  assert!(without_tags.tags().is_empty());
}
