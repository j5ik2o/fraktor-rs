use cellactor_utils_core_rs::sync::ArcShared;

use super::EventStreamSubscriptionGeneric;
use crate::{NoStdToolbox, eventstream::EventStream};

struct MockSubscriber;

impl crate::eventstream::EventStreamSubscriber<NoStdToolbox> for MockSubscriber {
  fn on_event(&self, _event: &crate::eventstream::EventStreamEvent<NoStdToolbox>) {}
}

#[test]
fn event_stream_subscription_new() {
  let stream = ArcShared::new(EventStream::default());
  let subscription = EventStreamSubscriptionGeneric::new(stream.clone(), 42);
  assert_eq!(subscription.id(), 42);
}

#[test]
fn event_stream_subscription_id() {
  let stream = ArcShared::new(EventStream::default());
  let subscription = EventStreamSubscriptionGeneric::new(stream.clone(), 100);
  assert_eq!(subscription.id(), 100);
}

#[test]
fn event_stream_subscription_drop_unsubscribes() {
  let stream = ArcShared::new(EventStream::default());
  let subscriber: ArcShared<dyn crate::eventstream::EventStreamSubscriber<NoStdToolbox>> =
    ArcShared::new(MockSubscriber);
  let subscription = EventStream::subscribe_arc(&stream, &subscriber);
  let id = subscription.id();

  // subscription?drop???unsubscribe?????
  drop(subscription);

  // ??subscribe??ID????????????unsubscribe??????
  let subscriber2: ArcShared<dyn crate::eventstream::EventStreamSubscriber<NoStdToolbox>> =
    ArcShared::new(MockSubscriber);
  let subscription2 = EventStream::subscribe_arc(&stream, &subscriber2);
  // ID???????????
  assert!(subscription2.id() > id);
}
