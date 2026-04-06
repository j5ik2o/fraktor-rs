//! ActorRef-based event stream subscriber for non-blocking event delivery.

#[cfg(test)]
mod tests;

use portable_atomic::{AtomicU64, Ordering};

use crate::core::kernel::{
  actor::{actor_ref::ActorRef, messaging::AnyMessage},
  event::stream::{EventStreamEvent, EventStreamSubscriber},
};

/// Event stream subscriber that forwards events to an ActorRef.
///
/// This enables **non-blocking publish()** by delegating event processing
/// to the actor's mailbox, similar to Akka/Pekko's `eventStream.subscribe(actorRef)`.
///
/// # Performance
///
/// - `publish()` returns immediately (only mailbox enqueue time)
/// - Event processing happens asynchronously in the actor
/// - Scales well with many subscribers (O(n) mailbox sends vs O(n) synchronous callbacks)
///
/// # Error Observability
///
/// Delivery failures (actor stopped, mailbox full) are counted in
/// [`failed_delivery_count`](Self::failed_delivery_count).
pub struct ActorRefEventStreamSubscriber {
  actor_ref:      ActorRef,
  failed_deliver: AtomicU64,
}

impl ActorRefEventStreamSubscriber {
  /// Creates a new subscriber that forwards events to the given ActorRef.
  #[must_use]
  pub const fn new(actor_ref: ActorRef) -> Self {
    Self { actor_ref, failed_deliver: AtomicU64::new(0) }
  }

  /// Returns a reference to the underlying ActorRef.
  #[must_use]
  pub const fn actor_ref(&self) -> &ActorRef {
    &self.actor_ref
  }

  /// Returns the number of events that failed to deliver since creation.
  #[must_use]
  pub fn failed_delivery_count(&self) -> u64 {
    self.failed_deliver.load(Ordering::Relaxed)
  }
}

impl EventStreamSubscriber for ActorRefEventStreamSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    let message = AnyMessage::new(event.clone());
    if self.actor_ref.try_tell(message).is_err() {
      self.failed_deliver.fetch_add(1, Ordering::Relaxed);
    }
  }
}
