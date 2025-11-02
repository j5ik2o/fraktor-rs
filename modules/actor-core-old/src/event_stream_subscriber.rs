//! Trait implemented by event stream subscribers.

use crate::event_stream_event::EventStreamEvent;

/// Observers registered with the event stream must implement this trait.
pub trait EventStreamSubscriber: Send + Sync {
  /// Invoked for every published event.
  fn on_event(&self, event: &EventStreamEvent);
}
