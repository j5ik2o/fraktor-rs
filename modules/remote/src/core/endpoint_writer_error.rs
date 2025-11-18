//! Error variants produced by [`EndpointWriter`](crate::core::endpoint_writer::EndpointWriter).

use fraktor_actor_rs::core::serialization::SerializationError;

use crate::core::outbound_priority::OutboundPriority;

/// Error raised when enqueueing or polling fails.
#[derive(Debug)]
pub enum EndpointWriterError {
  /// Queue rejected the message because it is full.
  QueueFull(OutboundPriority),
  /// Queue was closed or disconnected.
  QueueClosed(OutboundPriority),
  /// Queue reported an unexpected failure.
  QueueUnavailable {
    /// Priority of the queue that failed.
    priority: OutboundPriority,
    /// Description of the failure.
    reason:   &'static str,
  },
  /// Serialization failed for the message payload.
  Serialization(SerializationError),
}
