//! Inbound frame event published by [`crate::std::transport::tcp::TcpServer`] /
//! [`crate::std::transport::tcp::TcpClient`].

use alloc::string::String;

use bytes::Bytes;

use super::wire_frame::WireFrame;

/// Inbound event emitted when a [`crate::std::transport::tcp::WireFrame`]
/// arrives from a peer.
#[derive(Debug)]
pub struct InboundFrameEvent {
  /// Peer socket address (as a display-friendly string).
  pub peer:        String,
  /// Frame that was received.
  pub frame:       WireFrame,
  /// Original encoded bytes for the frame.
  pub frame_bytes: Bytes,
}
