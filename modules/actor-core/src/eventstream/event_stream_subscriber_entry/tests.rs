use cellactor_utils_core_rs::sync::ArcShared;

use super::EventStreamSubscriberEntry;
use crate::{NoStdToolbox, eventstream::EventStreamSubscriber};

struct MockSubscriber;

impl EventStreamSubscriber<NoStdToolbox> for MockSubscriber {
  fn on_event(&self, _event: &crate::eventstream::EventStreamEvent<NoStdToolbox>) {}
}

#[test]
fn event_stream_subscriber_entry_new() {
  let subscriber = ArcShared::new(MockSubscriber);
  let entry = EventStreamSubscriberEntry::new(42, subscriber.clone());
  assert_eq!(entry.id(), 42);
}

#[test]
fn event_stream_subscriber_entry_id() {
  let subscriber = ArcShared::new(MockSubscriber);
  let entry = EventStreamSubscriberEntry::new(100, subscriber.clone());
  assert_eq!(entry.id(), 100);
}

#[test]
fn event_stream_subscriber_entry_subscriber() {
  let subscriber = ArcShared::new(MockSubscriber);
  let entry = EventStreamSubscriberEntry::new(1, subscriber.clone());
  let retrieved = entry.subscriber();
  // ??subscriber?????????????????????
  let _ = retrieved;
}

#[test]
fn event_stream_subscriber_entry_clone() {
  let subscriber = ArcShared::new(MockSubscriber);
  let entry1 = EventStreamSubscriberEntry::new(5, subscriber.clone());
  let entry2 = entry1.clone();
  assert_eq!(entry1.id(), entry2.id());
  assert_eq!(entry1.id(), 5);
}
