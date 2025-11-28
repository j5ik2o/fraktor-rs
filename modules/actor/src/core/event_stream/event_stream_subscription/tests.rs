use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use super::EventStreamSubscription;
use crate::core::event_stream::{EventStream, EventStreamEvent, EventStreamSubscriber, subscriber_handle};

struct MockSubscriber;

impl EventStreamSubscriber<NoStdToolbox> for MockSubscriber {
  fn on_event(&mut self, _event: &EventStreamEvent<NoStdToolbox>) {}
}

#[test]
fn event_stream_subscription_new() {
  let stream = ArcShared::new(EventStream::default());
  let subscription = EventStreamSubscription::new(stream.clone(), 42);
  assert_eq!(subscription.id(), 42);
}

#[test]
fn event_stream_subscription_id() {
  let stream = ArcShared::new(EventStream::default());
  let subscription = EventStreamSubscription::new(stream.clone(), 100);
  assert_eq!(subscription.id(), 100);
}

#[test]
fn event_stream_subscription_drop_unsubscribes() {
  let stream = ArcShared::new(EventStream::default());
  let subscriber = subscriber_handle(MockSubscriber);
  let subscription = EventStream::subscribe_arc(&stream, &subscriber);
  let id = subscription.id();

  drop(subscription);

  let subscriber2 = subscriber_handle(MockSubscriber);
  let subscription2 = EventStream::subscribe_arc(&stream, &subscriber2);
  assert!(subscription2.id() > id);
}
