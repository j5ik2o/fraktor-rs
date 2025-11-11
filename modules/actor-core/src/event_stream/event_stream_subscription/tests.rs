use fraktor_utils_core_rs::sync::ArcShared;

use super::EventStreamSubscription;
use crate::{NoStdToolbox, event_stream::EventStream};

struct MockSubscriber;

impl crate::event_stream::EventStreamSubscriber<NoStdToolbox> for MockSubscriber {
  fn on_event(&self, _event: &crate::event_stream::EventStreamEvent<NoStdToolbox>) {}
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
  let subscriber: ArcShared<dyn crate::event_stream::EventStreamSubscriber<NoStdToolbox>> =
    ArcShared::new(MockSubscriber);
  let subscription = EventStream::subscribe_arc(&stream, &subscriber);
  let id = subscription.id();

  drop(subscription);

  let subscriber2: ArcShared<dyn crate::event_stream::EventStreamSubscriber<NoStdToolbox>> =
    ArcShared::new(MockSubscriber);
  let subscription2 = EventStream::subscribe_arc(&stream, &subscriber2);
  assert!(subscription2.id() > id);
}
