//! Outcome of a [`crate::core::association::SendQueue::offer`] call.

use alloc::boxed::Box;

use crate::core::envelope::OutboundEnvelope;

/// Outcome of queueing an envelope into a [`crate::core::association::SendQueue`].
///
/// Queue-full outcomes carry the rejected envelope back to the caller so the
/// association can make the discard observable instead of silently dropping it.
#[derive(Debug)]
pub enum OfferOutcome {
  /// The envelope was appended to the queue.
  Accepted,
  /// The matching priority lane was full and the envelope was not queued.
  QueueFull {
    /// Envelope rejected by the bounded lane.
    envelope: Box<OutboundEnvelope>,
  },
}
