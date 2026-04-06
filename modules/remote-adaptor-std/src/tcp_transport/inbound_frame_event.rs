//! Inbound frame event published by [`crate::tcp_transport::TcpServer`] /
//! [`crate::tcp_transport::TcpClient`].

use alloc::string::String;

use crate::tcp_transport::wire_frame::WireFrame;

/// Inbound event emitted when a [`crate::tcp_transport::WireFrame`]
/// arrives from a peer.
#[derive(Debug)]
pub struct InboundFrameEvent {
  /// Peer socket address (as a display-friendly string).
  pub peer:  String,
  /// Frame that was received.
  pub frame: WireFrame,
}
