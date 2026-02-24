//! Marker contract for remoting control messages.

/// Marker contract for messages delivered on the remoting control sub-channel.
pub trait ControlMessage: Send + Sync + 'static {
  /// Returns the wire frame kind associated with the control message.
  fn frame_kind(&self) -> u8;
}
