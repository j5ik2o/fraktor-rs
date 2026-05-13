use super::EventStreamSubscriberEntry;
use crate::event::stream::{ClassifierKey, EventStreamEvent, EventStreamSubscriber, tests::subscriber_handle};

struct MockSubscriber;

impl EventStreamSubscriber for MockSubscriber {
  fn on_event(&mut self, _event: &EventStreamEvent) {}
}

#[test]
fn event_stream_subscriber_entry_new() {
  let subscriber = subscriber_handle(MockSubscriber);
  let entry = EventStreamSubscriberEntry::new(42, ClassifierKey::Log, subscriber.clone());
  assert_eq!(entry.id(), 42);
  assert_eq!(entry.key(), ClassifierKey::Log);
}

#[test]
fn event_stream_subscriber_entry_id() {
  let subscriber = subscriber_handle(MockSubscriber);
  let entry = EventStreamSubscriberEntry::new(100, ClassifierKey::All, subscriber.clone());
  assert_eq!(entry.id(), 100);
}

#[test]
fn event_stream_subscriber_entry_key() {
  let subscriber = subscriber_handle(MockSubscriber);
  let entry = EventStreamSubscriberEntry::new(7, ClassifierKey::Lifecycle, subscriber.clone());
  assert_eq!(entry.key(), ClassifierKey::Lifecycle);
}

#[test]
fn event_stream_subscriber_entry_subscriber() {
  let subscriber = subscriber_handle(MockSubscriber);
  let entry = EventStreamSubscriberEntry::new(1, ClassifierKey::DeadLetter, subscriber.clone());
  let retrieved = entry.subscriber();
  let _ = retrieved;
}

#[test]
fn event_stream_subscriber_entry_clone() {
  let subscriber = subscriber_handle(MockSubscriber);
  let entry1 = EventStreamSubscriberEntry::new(5, ClassifierKey::Extension, subscriber.clone());
  let entry2 = entry1.clone();
  assert_eq!(entry1.id(), entry2.id());
  assert_eq!(entry1.key(), entry2.key());
  assert_eq!(entry1.id(), 5);
}
