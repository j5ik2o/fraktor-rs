//! Outcome of a [`crate::core::association::SendQueue::offer`] call.

/// Outcome of queueing an envelope into a [`crate::core::association::SendQueue`].
///
/// Phase A only needs the `Accepted` variant — the queue is unbounded under
/// Phase A semantics. The enum is kept so that Phase B can add backpressure
/// variants (`Rejected`, `QueueFull`) without breaking the API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OfferOutcome {
  /// The envelope was appended to the queue.
  Accepted,
}
