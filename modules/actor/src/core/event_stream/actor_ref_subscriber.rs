//! ActorRef-based event stream subscriber for non-blocking event delivery.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  actor_prim::actor_ref::ActorRefGeneric,
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  messaging::AnyMessageGeneric,
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
pub struct ActorRefEventStreamSubscriber<TB: RuntimeToolbox + 'static> {
  actor_ref: ActorRefGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> ActorRefEventStreamSubscriber<TB> {
  /// Creates a new subscriber that forwards events to the given ActorRef.
  #[must_use]
  pub const fn new(actor_ref: ActorRefGeneric<TB>) -> Self {
    Self { actor_ref }
  }

  /// Returns a reference to the underlying ActorRef.
  #[must_use]
  pub const fn actor_ref(&self) -> &ActorRefGeneric<TB> {
    &self.actor_ref
  }
}

impl<TB: RuntimeToolbox + 'static> EventStreamSubscriber<TB> for ActorRefEventStreamSubscriber<TB> {
  fn on_event(&mut self, event: &EventStreamEvent<TB>) {
    // Non-blocking message send to actor's mailbox
    let message = AnyMessageGeneric::new(event.clone());
    let _ = self.actor_ref.tell(message);
    // Errors are silently ignored (actor may be stopped, mailbox full, etc.)
    // This matches Akka/Pekko behavior where dead letter handling is separate
  }
}
