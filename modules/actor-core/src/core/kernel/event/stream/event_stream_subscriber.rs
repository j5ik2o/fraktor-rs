//! Trait implemented by event stream observers.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedLock};

use crate::core::kernel::{event::stream::EventStreamEvent, system::shared_factory::ActorSharedFactory};

/// Shared subscriber handle guarded by the runtime mutex family.
pub type EventStreamSubscriberShared = SharedLock<Box<dyn EventStreamSubscriber>>;

/// Observers registered with the event stream must implement this trait.
pub trait EventStreamSubscriber: Send + Sync + 'static {
  /// Invoked for every published event.
  fn on_event(&mut self, event: &EventStreamEvent);
}

/// Wraps the subscriber with the actor-system scoped shared factory.
#[must_use]
pub fn subscriber_handle_with_shared_factory(
  lock_provider: &ArcShared<dyn ActorSharedFactory>,
  subscriber: impl EventStreamSubscriber,
) -> EventStreamSubscriberShared {
  lock_provider.create_event_stream_subscriber_shared(Box::new(subscriber))
}
