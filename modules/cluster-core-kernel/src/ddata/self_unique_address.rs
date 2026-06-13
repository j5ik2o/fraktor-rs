//! Explicit self-node identity for node-local CRDT updates.

#[cfg(test)]
#[path = "self_unique_address_test.rs"]
mod tests;

use fraktor_remote_core_rs::address::UniqueAddress;

/// Newtype that marks the local node address used by CRDT updates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SelfUniqueAddress {
  unique_address: UniqueAddress,
}

impl SelfUniqueAddress {
  /// Creates a new local-node identity wrapper.
  #[must_use]
  pub const fn new(unique_address: UniqueAddress) -> Self {
    Self { unique_address }
  }

  /// Returns the wrapped unique address.
  #[must_use]
  pub const fn unique_address(&self) -> &UniqueAddress {
    &self.unique_address
  }
}
