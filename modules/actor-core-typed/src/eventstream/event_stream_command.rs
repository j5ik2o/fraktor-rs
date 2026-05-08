//! Command type for the typed system event stream.

#[cfg(test)]
mod tests;

use fraktor_actor_core_rs::core::kernel::{actor::actor_ref::ActorRef, event::stream::EventStreamEvent};

/// Commands accepted by the typed event stream, mirroring Pekko's `EventStream.Command`.
pub enum EventStreamCommand {
  /// Publishes an event to all subscribers.
  Publish(EventStreamEvent),
  /// Subscribes an actor to receive event stream events.
  ///
  /// Corresponds to Pekko's `EventStream.Subscribe`.
  Subscribe {
    /// The subscriber actor reference.
    subscriber: ActorRef,
  },
  /// Unsubscribes an actor from the event stream.
  ///
  /// Corresponds to Pekko's `EventStream.Unsubscribe`.
  Unsubscribe {
    /// The subscriber actor reference to remove.
    subscriber: ActorRef,
  },
}
