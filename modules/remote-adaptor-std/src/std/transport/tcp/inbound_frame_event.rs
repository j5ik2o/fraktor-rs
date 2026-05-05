//! Inbound frame event published by [`crate::std::transport::tcp::TcpServer`] /
//! [`crate::std::transport::tcp::TcpClient`].

use alloc::string::String;

use super::WireFrame;

/// Inbound event emitted when a [`crate::std::transport::tcp::WireFrame`]
/// arrives from a peer.
#[derive(Debug)]
pub struct InboundFrameEvent {
  /// Peer socket address (as a display-friendly string).
  pub peer:  String,
  /// Frame that was received.
  pub frame: WireFrame,
}
