//! Configuration for Tokio gossip transport.

use fraktor_remote_core_rs::address::UniqueAddress;

/// Configuration for Tokio gossip transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokioGossipTransportConfig {
  /// UDP bind address (e.g. "127.0.0.1:0").
  pub bind_addr:               String,
  /// Maximum datagram size in bytes.
  pub max_datagram_bytes:      usize,
  /// Outbound queue capacity.
  pub outbound_capacity:       usize,
  /// Trusted remote peers allowed to send gossip datagrams.
  pub allowed_peers:           Vec<String>,
  /// Local peer identity used to validate inbound logical envelope handoff.
  pub local_identity:          Option<UniqueAddress>,
  /// Trusted remote peer identities used by logical envelope handoff.
  pub allowed_peer_identities: Vec<UniqueAddress>,
}

impl TokioGossipTransportConfig {
  /// Creates a new configuration.
  #[must_use]
  pub fn new(bind_addr: String, max_datagram_bytes: usize, outbound_capacity: usize) -> Self {
    Self {
      bind_addr,
      max_datagram_bytes,
      outbound_capacity,
      allowed_peers: Vec::new(),
      local_identity: None,
      allowed_peer_identities: Vec::new(),
    }
  }

  /// Adds trusted remote peers.
  #[must_use]
  pub fn with_allowed_peers(mut self, allowed_peers: Vec<String>) -> Self {
    self.allowed_peers = allowed_peers;
    self
  }

  /// Sets the local peer identity for inbound envelope handoff validation.
  #[must_use]
  pub fn with_local_identity(mut self, local_identity: UniqueAddress) -> Self {
    self.local_identity = Some(local_identity);
    self
  }

  /// Adds trusted remote peer identities for envelope handoff.
  #[must_use]
  pub fn with_allowed_peer_identities(mut self, allowed_peer_identities: Vec<UniqueAddress>) -> Self {
    self.allowed_peer_identities = allowed_peer_identities;
    self
  }
}
