//! Trait implemented by event stream observers.

use alloc::boxed::Box;

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeMutex, sync::ArcShared};

use crate::core::event::stream::EventStreamEvent;

/// Shared subscriber handle guarded by the runtime mutex family.
pub type EventStreamSubscriberShared = ArcShared<RuntimeMutex<Box<dyn EventStreamSubscriber>>>;

/// Observers registered with the event stream must implement this trait.
pub trait EventStreamSubscriber: Send + Sync + 'static {
  /// Invoked for every published event.
  fn on_event(&mut self, event: &EventStreamEvent);
}

/// Wraps the subscriber into a mutex-protected shared handle.
#[must_use]
pub fn subscriber_handle(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  ArcShared::new(RuntimeMutex::new(Box::new(subscriber)))
}
