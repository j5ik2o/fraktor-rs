use cellactor_actor_core_rs::eventstream::{
  EventStreamEvent as CoreEventStreamEvent, EventStreamGeneric as CoreEventStream,
  EventStreamSubscriber as CoreEventStreamSubscriber, EventStreamSubscriptionGeneric as CoreEventStreamSubscription,
};
use cellactor_utils_core_rs::sync::ArcShared;
use cellactor_utils_std_rs::StdToolbox;

use crate::system::ActorSystem;

/// Event stream specialised for `StdToolbox`.
pub type EventStream = CoreEventStream<StdToolbox>;
/// Event stream event specialised for `StdToolbox`.
pub type EventStreamEvent = CoreEventStreamEvent<StdToolbox>;
/// Event stream subscription specialised for `StdToolbox`.
pub type EventStreamSubscription = CoreEventStreamSubscription<StdToolbox>;

/// Trait implemented by observers interested in the standard runtime event stream.
pub trait EventStreamSubscriber: Send + Sync + 'static {
  /// Receives a published event.
  fn on_event(&self, event: &EventStreamEvent);
}

impl<T> EventStreamSubscriber for T
where
  T: CoreEventStreamSubscriber<StdToolbox>,
{
  fn on_event(&self, event: &EventStreamEvent) {
    CoreEventStreamSubscriber::on_event(self, event)
  }
}

struct EventStreamSubscriberAdapter {
  inner: ArcShared<dyn EventStreamSubscriber>,
}

impl EventStreamSubscriberAdapter {
  const fn new(inner: ArcShared<dyn EventStreamSubscriber>) -> Self {
    Self { inner }
  }
}

impl CoreEventStreamSubscriber<StdToolbox> for EventStreamSubscriberAdapter {
  fn on_event(&self, event: &EventStreamEvent) {
    self.inner.on_event(event);
  }
}

/// Subscribes an observer implemented using [`EventStreamSubscriber`].
#[must_use]
pub fn subscribe(system: &ActorSystem, subscriber: &ArcShared<dyn EventStreamSubscriber>) -> EventStreamSubscription {
  let adapter: ArcShared<dyn CoreEventStreamSubscriber<StdToolbox>> =
    ArcShared::new(EventStreamSubscriberAdapter::new(subscriber.clone()));
  system.as_core().subscribe_event_stream(&adapter)
}
