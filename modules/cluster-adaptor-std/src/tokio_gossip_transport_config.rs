//! Configuration for Tokio gossip transport.

/// Configuration for Tokio gossip transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokioGossipTransportConfig {
  /// UDP bind address (e.g. "127.0.0.1:0").
  pub bind_addr:          String,
  /// Maximum datagram size in bytes.
  pub max_datagram_bytes: usize,
  /// Outbound queue capacity.
  pub outbound_capacity:  usize,
  /// Trusted remote peers allowed to send gossip datagrams.
  pub allowed_peers:      Vec<String>,
}

impl TokioGossipTransportConfig {
  /// Creates a new configuration.
  #[must_use]
  pub fn new(bind_addr: String, max_datagram_bytes: usize, outbound_capacity: usize) -> Self {
    Self { bind_addr, max_datagram_bytes, outbound_capacity, allowed_peers: Vec::new() }
  }

  /// Adds trusted remote peers.
  #[must_use]
  pub fn with_allowed_peers(mut self, allowed_peers: Vec<String>) -> Self {
    self.allowed_peers = allowed_peers;
    self
  }
}
