//! Trait implemented by event stream observers.

use alloc::boxed::Box;

use crate::core::kernel::event::stream::{EventStreamEvent, EventStreamSubscriberShared};

/// Observers registered with the event stream must implement this trait.
pub trait EventStreamSubscriber: Send + Sync + 'static {
  /// Invoked for every published event.
  fn on_event(&mut self, event: &EventStreamEvent);
}

/// Wraps a subscriber into a shared handle using direct construction.
#[must_use]
pub fn subscriber_handle_with_shared_factory(subscriber: impl EventStreamSubscriber) -> EventStreamSubscriberShared {
  EventStreamSubscriberShared::new(Box::new(subscriber))
}
