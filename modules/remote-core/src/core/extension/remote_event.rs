//! Event values consumed by [`crate::core::extension::Remote::run`].

use alloc::{boxed::Box, vec::Vec};

use crate::core::{
  envelope::OutboundEnvelope,
  transport::{TransportEndpoint, TransportError as ConnectionLostCause},
};

/// Events pushed by adapter code and consumed by the core remote event loop.
#[derive(Debug)]
pub enum RemoteEvent {
  /// A raw inbound frame was received from `authority`.
  InboundFrameReceived {
    /// Remote authority that produced the frame.
    authority: TransportEndpoint,
    /// Raw frame bytes.
    frame:     Vec<u8>,
  },
  /// A generation-scoped handshake timer fired.
  HandshakeTimerFired {
    /// Remote authority whose timer fired.
    authority:  TransportEndpoint,
    /// Handshake generation carried by the scheduled timer.
    generation: u64,
  },
  /// An outbound envelope has been submitted by adapter code.
  OutboundEnqueued {
    /// Remote authority that should receive the envelope.
    authority: TransportEndpoint,
    /// Envelope to enqueue and drain.
    envelope:  Box<OutboundEnvelope>,
  },
  /// A transport connection was lost.
  ConnectionLost {
    /// Remote authority whose connection was lost.
    authority: TransportEndpoint,
    /// Transport-level cause.
    cause:     ConnectionLostCause,
  },
  /// The transport should stop the event loop.
  TransportShutdown,
}
