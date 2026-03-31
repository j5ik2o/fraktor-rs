//! Command type for the typed system event stream.

use crate::core::kernel::event::stream::EventStreamEvent;

/// Commands accepted by the typed event stream, mirroring Pekko's `EventStream.Command`.
pub enum EventStreamCommand {
  /// Publishes an event to all subscribers.
  Publish(EventStreamEvent),
}
