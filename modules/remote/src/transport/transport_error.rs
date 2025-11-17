//! Errors emitted by transport operations.

/// Transport-level errors.
#[derive(Debug, PartialEq, Eq)]
pub enum TransportError {
  /// Listener binding failed.
  BindFailed,
  /// Opening a channel failed.
  ChannelUnavailable,
  /// Payload could not be delivered.
  SendFailed,
}
