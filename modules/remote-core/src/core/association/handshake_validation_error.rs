//! Handshake validation errors.

use crate::core::address::Address;

/// Error returned when a handshake message does not match the association endpoints.
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
}
