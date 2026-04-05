use alloc::boxed::Box;

use fraktor_actor_rs::core::kernel::event::stream::EventStreamEvent;
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

/// Trait implemented by observers interested in the standard runtime event stream.
pub trait EventStreamSubscriber: Send + Sync + 'static {
  /// Receives a published event.
  fn on_event(&mut self, event: &EventStreamEvent);
}

/// Shared handle protected by the runtime-selected mutex.
pub type EventStreamSubscriberShared = ArcShared<RuntimeMutex<Box<dyn EventStreamSubscriber>>>;

/// Wraps the subscriber into a mutex-protected shared handle for the standard runtime.
#[must_use]
pub fn subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  ArcShared::new(RuntimeMutex::new(Box::new(subscriber)))
}
