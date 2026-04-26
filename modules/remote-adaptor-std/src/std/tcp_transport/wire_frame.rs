//! Unified on-the-wire frame used by the adapter layer.

use fraktor_remote_core_rs::core::wire::{AckPdu, ControlPdu, EnvelopePdu, HandshakePdu};

/// Unified wire frame multiplexing every PDU kind over a single tokio `Framed`
/// stream.
///
/// The adapter layer reads and writes `WireFrame` values; individual PDU types
/// from `remote-core` are wrapped/unwrapped as needed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireFrame {
  /// Envelope PDU carrying a user / system message.
  Envelope(EnvelopePdu),
  /// Handshake request or response.
  Handshake(HandshakePdu),
  /// Control message (heartbeat / quarantine / shutdown).
  Control(ControlPdu),
  /// System message delivery ack.
  Ack(AckPdu),
}
