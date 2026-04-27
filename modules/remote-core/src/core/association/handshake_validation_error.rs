//! Handshake validation errors.

use core::fmt::{Display, Formatter, Result as FmtResult};

use crate::core::address::Address;

/// Error returned when a handshake message cannot be accepted by this association.
///
/// Either the wire-level endpoints fail validation, or the local association is
/// in a state (Idle / Gated / Quarantined) where it must not advertise itself
/// as Active to the remote peer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandshakeValidationError {
  /// The request was addressed to a different local address.
  UnexpectedDestination {
    /// Local address expected by this association.
    expected: Address,
    /// Local address carried by the request.
    actual:   Address,
  },
  /// The request or response came from a different remote address.
  UnexpectedRemote {
    /// Remote address expected by this association.
    expected: Address,
    /// Remote address carried by the handshake message.
    actual:   Address,
  },
  /// The local association is in a state that must not be silently promoted to
  /// `Active` by an inbound handshake (e.g. `Idle`, `Gated`, `Quarantined`).
  ///
  /// The dispatcher must propagate this as a no-response so the remote peer
  /// does not believe the handshake succeeded while the local side stays
  /// unreachable.
  RejectedInState {
    /// Discriminator of the state at the time of rejection (e.g. `"Idle"`).
    state: &'static str,
  },
}

impl Display for HandshakeValidationError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::UnexpectedDestination { expected, actual } => {
        write!(f, "handshake validation: unexpected destination (expected {expected}, got {actual})")
      },
      | Self::UnexpectedRemote { expected, actual } => {
        write!(f, "handshake validation: unexpected remote (expected {expected}, got {actual})")
      },
      | Self::RejectedInState { state } => {
        write!(f, "handshake validation: rejected in state {state}")
      },
    }
  }
}

impl core::error::Error for HandshakeValidationError {}
