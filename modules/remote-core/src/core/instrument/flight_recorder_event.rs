//! Flight recorder event variants.

use alloc::string::String;

use fraktor_actor_core_rs::event::stream::CorrelationId;

use crate::core::{instrument::handshake_phase::HandshakePhase, transport::BackpressureSignal};

/// Event recorded by [`crate::core::instrument::RemotingFlightRecorder`].
///
/// Every variant carries a `now_ms` field containing the **monotonic millis**
/// timestamp assigned by the caller (see design Decision 7). This keeps the
/// snapshot ordering stable across wall-clock jumps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlightRecorderEvent {
  /// An envelope was handed to the transport for sending.
  Send {
    /// Authority of the destination node.
    authority:      String,
    /// Correlation id attached to the envelope.
    correlation_id: CorrelationId,
    /// Priority byte (`0 = System`, `1 = User`).
    priority:       u8,
    /// Encoded payload size in bytes.
    size:           u32,
    /// Monotonic millis at which the event occurred.
    now_ms:         u64,
  },
  /// An outbound envelope was discarded after an unrecoverable send failure.
  DroppedEnvelope {
    /// Authority of the destination node.
    authority:      String,
    /// Correlation id attached to the discarded envelope.
    correlation_id: CorrelationId,
    /// Priority byte (`0 = System`, `1 = User`).
    priority:       u8,
    /// Monotonic millis at which the event occurred.
    now_ms:         u64,
  },
  /// An envelope was decoded from an inbound frame.
  Receive {
    /// Authority of the source node.
    authority:      String,
    /// Correlation id attached to the envelope.
    correlation_id: CorrelationId,
    /// Encoded payload size in bytes.
    size:           u32,
    /// Monotonic millis at which the event occurred.
    now_ms:         u64,
  },
  /// A handshake transitioned through one of its lifecycle phases.
  Handshake {
    /// Authority of the remote peer.
    authority: String,
    /// Lifecycle phase reached.
    phase:     HandshakePhase,
    /// Monotonic millis at which the event occurred.
    now_ms:    u64,
  },
  /// A remote peer has been quarantined.
  Quarantine {
    /// Authority of the quarantined peer.
    authority: String,
    /// Human-readable reason.
    reason:    String,
    /// Monotonic millis at which the event occurred.
    now_ms:    u64,
  },
  /// A backpressure signal was observed on a given link.
  Backpressure {
    /// Authority of the peer whose link is being throttled.
    authority:      String,
    /// Direction of the signal (`Apply` / `Release`).
    signal:         BackpressureSignal,
    /// Correlation id linking the signal to downstream telemetry.
    correlation_id: CorrelationId,
    /// Monotonic millis at which the event occurred.
    now_ms:         u64,
  },
}
