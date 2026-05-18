//! Inbound frame event published by [`crate::transport::tcp::TcpServer`] /
//! [`crate::transport::tcp::TcpClient`].

use alloc::string::String;

use fraktor_remote_core_rs::transport::TransportEndpoint;

use super::WireFrame;

/// Inbound event emitted when a [`crate::transport::tcp::WireFrame`]
/// arrives from a peer.
#[derive(Debug)]
pub struct InboundFrameEvent {
  /// Peer socket address (as a display-friendly string).
  pub peer:      String,
  /// Remote authority learned from previous frames on this connection.
  pub authority: Option<TransportEndpoint>,
  /// Frame that was received.
  pub frame:     WireFrame,
}
