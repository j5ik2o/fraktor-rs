//! Event stream bindings for standard runtimes.

mod dead_letter_log_subscriber;

pub use dead_letter_log_subscriber::DeadLetterLogSubscriber;
use fraktor_actor_rs::core::kernel::event::stream::{
  EventStreamSubscriber as CoreEventStreamSubscriber, EventStreamSubscriberShared as CoreEventStreamSubscriberShared,
};

/// Shared handle protected by the runtime-selected mutex.
pub type EventStreamSubscriberShared = CoreEventStreamSubscriberShared;

/// Wraps the subscriber into a mutex-protected shared handle for the standard runtime.
#[must_use]
pub fn subscriber_handle(subscriber: impl CoreEventStreamSubscriber) -> EventStreamSubscriberShared {
  fraktor_actor_rs::core::kernel::event::stream::subscriber_handle(subscriber)
}
