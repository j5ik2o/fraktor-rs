//! Schema version negotiation used during RPC handshake.

use alloc::vec::Vec;

#[cfg(test)]
mod tests;

/// Negotiates a common schema version with peers.
pub struct SchemaNegotiator {
  supported: Vec<u32>,
}

impl SchemaNegotiator {
  /// Creates a negotiator with supported versions.
  #[must_use]
  pub const fn new(supported: Vec<u32>) -> Self {
    Self { supported }
  }

  /// Returns the highest common version or None if incompatible.
  #[must_use]
  pub fn negotiate(&self, peer_supported: &[u32]) -> Option<u32> {
    self.supported.iter().copied().filter(|v| peer_supported.contains(v)).max()
  }

  /// Returns supported versions.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub fn supported(&self) -> &[u32] {
    &self.supported
  }
}
