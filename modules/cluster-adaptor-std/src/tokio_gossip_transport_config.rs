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
}

impl TokioGossipTransportConfig {
  /// Creates a new configuration.
  #[must_use]
  pub const fn new(bind_addr: String, max_datagram_bytes: usize, outbound_capacity: usize) -> Self {
    Self { bind_addr, max_datagram_bytes, outbound_capacity }
  }
}
