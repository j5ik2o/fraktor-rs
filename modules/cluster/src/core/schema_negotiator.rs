//! Schema version negotiation used during RPC handshake.

use alloc::vec::Vec;

/// Negotiates a common schema version with peers.
pub struct SchemaNegotiator {
  supported: Vec<u32>,
}

impl SchemaNegotiator {
  /// Creates a negotiator with supported versions.
  pub fn new(supported: Vec<u32>) -> Self {
    Self { supported }
  }

  /// Returns the highest common version or None if incompatible.
  pub fn negotiate(&self, peer_supported: &[u32]) -> Option<u32> {
    self
      .supported
      .iter()
      .copied()
      .filter(|v| peer_supported.contains(v))
      .max()
  }

  /// Returns supported versions.
  pub fn supported(&self) -> &[u32] {
    &self.supported
  }
}

#[cfg(test)]
mod tests;
