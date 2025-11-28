use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use super::EventStreamSubscriberEntry;
use crate::core::event_stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle};

struct MockSubscriber;

impl EventStreamSubscriber for MockSubscriber {
  fn on_event(&mut self, _event: &EventStreamEvent<NoStdToolbox>) {}
}

#[test]
fn event_stream_subscriber_entry_new() {
  let subscriber = subscriber_handle(MockSubscriber);
  let entry = EventStreamSubscriberEntry::new(42, subscriber.clone());
  assert_eq!(entry.id(), 42);
}

#[test]
fn event_stream_subscriber_entry_id() {
  let subscriber = subscriber_handle(MockSubscriber);
  let entry = EventStreamSubscriberEntry::new(100, subscriber.clone());
  assert_eq!(entry.id(), 100);
}

#[test]
fn event_stream_subscriber_entry_subscriber() {
  let subscriber = subscriber_handle(MockSubscriber);
  let entry = EventStreamSubscriberEntry::new(1, subscriber.clone());
  let retrieved = entry.subscriber();
  let _ = retrieved;
}

#[test]
fn event_stream_subscriber_entry_clone() {
  let subscriber = subscriber_handle(MockSubscriber);
  let entry1 = EventStreamSubscriberEntry::new(5, subscriber.clone());
  let entry2 = entry1.clone();
  assert_eq!(entry1.id(), entry2.id());
  assert_eq!(entry1.id(), 5);
}
