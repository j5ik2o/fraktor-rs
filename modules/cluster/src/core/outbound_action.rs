//! Outcomes produced by outbound pipeline operations.

use alloc::string::String;

use crate::core::outbound_envelope::OutboundEnvelope;

/// Result of attempting to send an outbound envelope.
#[derive(Debug, PartialEq, Eq)]
pub enum OutboundAction {
  /// Envelope should be dispatched immediately (Connected state).
  Immediate {
    /// Envelope ready for dispatch.
    envelope: OutboundEnvelope,
  },
  /// Envelope was buffered while disconnected.
  Enqueued {
    /// Queue length after enqueue.
    queue_len: usize,
  },
  /// Oldest message was dropped to make room for the new one.
  DroppedOldest {
    /// Envelope that was discarded as DeadLetter.
    dropped: OutboundEnvelope,
    /// Queue length after replacing the oldest entry.
    queue_len: usize,
  },
  /// Send was rejected because the authority is quarantined.
  RejectedQuarantine {
    /// Human-readable reason.
    reason: String,
  },
}

#[cfg(test)]
mod tests;
