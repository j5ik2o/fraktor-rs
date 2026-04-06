//! Errors raised by [`crate::transport::RemoteTransport`] implementations.

use core::fmt;

/// Failure categories surfaced by a [`crate::transport::RemoteTransport`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
  /// The requested URI scheme is not supported by this transport.
  UnsupportedScheme,
  /// The transport is not available in the current environment (e.g. missing
  /// network stack).
  NotAvailable,
  /// `start` was called while the transport was already running.
  AlreadyRunning,
  /// A lifecycle operation was attempted before `start` succeeded.
  NotStarted,
  /// The transport failed to hand a message to the peer.
  SendFailed,
  /// A previously established connection has been closed.
  ConnectionClosed,
}

impl fmt::Display for TransportError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | TransportError::UnsupportedScheme => f.write_str("transport: unsupported scheme"),
      | TransportError::NotAvailable => f.write_str("transport: not available"),
      | TransportError::AlreadyRunning => f.write_str("transport: already running"),
      | TransportError::NotStarted => f.write_str("transport: not started"),
      | TransportError::SendFailed => f.write_str("transport: send failed"),
      | TransportError::ConnectionClosed => f.write_str("transport: connection closed"),
    }
  }
}

impl core::error::Error for TransportError {}
