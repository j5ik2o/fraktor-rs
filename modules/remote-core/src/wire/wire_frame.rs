//! Decoded wire frame variants exchanged between transport adapters and core remoting.

use crate::wire::{AckPdu, ControlPdu, EnvelopePdu, HandshakePdu, RemoteDeploymentPdu};

/// Decoded on-the-wire frame consumed by the core remote event loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireFrame {
  /// Envelope PDU carrying a user or system message.
  Envelope(EnvelopePdu),
  /// Handshake request or response.
  Handshake(HandshakePdu),
  /// Control message such as heartbeat, quarantine, or shutdown.
  Control(ControlPdu),
  /// System message delivery acknowledgement.
  Ack(AckPdu),
  /// Remote deployment create request or response.
  Deployment(RemoteDeploymentPdu),
}
