//! Enumerates handshake lifecycle phases for flight recorder events.

/// Lifecycle phase of a handshake, recorded by the flight recorder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HandshakePhase {
  /// Handshake has been initiated (Req sent).
  Started,
  /// Handshake completed successfully (Rsp received).
  Accepted,
  /// Handshake was rejected or timed out before completion.
  Rejected,
}
