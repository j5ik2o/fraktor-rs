//! Event values consumed by [`crate::extension::Remote::run`].

use alloc::boxed::Box;

use crate::{
  address::Address,
  envelope::OutboundEnvelope,
  transport::{TransportEndpoint, TransportError as ConnectionLostCause},
  wire::{ControlPdu, WireFrame},
};

/// Events pushed by adapter code and consumed by the core remote event loop.
#[derive(Debug)]
pub enum RemoteEvent {
  /// An inbound frame was received from `authority`.
  InboundFrameReceived {
    /// Remote authority that produced the frame.
    authority: TransportEndpoint,
    /// Decoded frame.
    frame:     WireFrame,
    /// Monotonic millis at which the frame was observed.
    now_ms:    u64,
  },
  /// A generation-scoped handshake timer fired.
  HandshakeTimerFired {
    /// Remote authority whose timer fired.
    authority:  TransportEndpoint,
    /// Handshake generation carried by the scheduled timer.
    generation: u64,
    /// Monotonic millis at which the timer fired.
    now_ms:     u64,
  },
  /// A flush session deadline was reached.
  FlushTimerFired {
    /// Remote authority whose flush timer fired.
    authority: TransportEndpoint,
    /// Flush session identifier carried by the scheduled timer.
    flush_id:  u64,
    /// Monotonic millis at which the timer fired.
    now_ms:    u64,
  },
  /// An outbound envelope has been submitted by adapter code.
  OutboundEnqueued {
    /// Remote authority that should receive the envelope.
    authority: TransportEndpoint,
    /// Envelope to enqueue and drain.
    envelope:  Box<OutboundEnvelope>,
    /// Monotonic millis at which the outbound envelope was observed.
    now_ms:    u64,
  },
  /// An outbound control PDU has been submitted by adapter code.
  OutboundControl {
    /// Remote address that should receive the control PDU.
    remote: Address,
    /// Control PDU to send.
    pdu:    ControlPdu,
    /// Monotonic millis at which the outbound control PDU was observed.
    now_ms: u64,
  },
  /// A redelivery timer fired for a remote authority.
  RedeliveryTimerFired {
    /// Remote authority whose pending system envelopes should be checked.
    authority: TransportEndpoint,
    /// Monotonic millis at which the timer fired.
    now_ms:    u64,
  },
  /// A transport connection was lost.
  ConnectionLost {
    /// Remote authority whose connection was lost.
    authority: TransportEndpoint,
    /// Transport-level cause.
    cause:     ConnectionLostCause,
    /// Monotonic millis at which the loss was observed.
    now_ms:    u64,
  },
  /// The transport should stop the event loop.
  TransportShutdown,
}
