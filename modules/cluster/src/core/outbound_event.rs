//! Events emitted from the outbound pipeline.

use alloc::string::String;

use crate::core::outbound_envelope::OutboundEnvelope;

#[cfg(test)]
mod tests;

/// Event kinds that feed EventStream/metrics.
#[derive(Debug, PartialEq, Eq)]
pub enum OutboundEvent {
  /// Message was enqueued while disconnected.
  Enqueued {
    /// Target PID string.
    pid:       String,
    /// Queue length after enqueue.
    queue_len: usize,
  },
  /// Oldest message was dropped as DeadLetter.
  DroppedOldest {
    /// Envelope that got discarded.
    dropped: OutboundEnvelope,
    /// Reason for the drop.
    reason:  String,
  },
  /// Buffered messages were flushed after reconnection.
  Flushed {
    /// Number of messages delivered from the buffer.
    delivered: usize,
  },
  /// Message dispatch succeeded immediately.
  Dispatched {
    /// PID that was dispatched without buffering.
    pid: String,
  },
  /// Send was blocked by quarantine.
  BlockedByQuarantine {
    /// PID that got rejected.
    pid:    String,
    /// Quarantine reason.
    reason: String,
  },
  /// Authority entered quarantine.
  Quarantined {
    /// Target authority string.
    authority: String,
    /// Reason for entering quarantine.
    reason:    String,
    /// Optional deadline when it should lift automatically.
    deadline:  Option<u64>,
  },
  /// Quarantine was lifted.
  QuarantineLifted {
    /// Authority whose quarantine was lifted.
    authority: String,
  },
  /// Serialization failure was detected before send.
  SerializationFailed {
    /// PID associated with the failure.
    pid:    String,
    /// Message describing the failure.
    reason: String,
  },
}
