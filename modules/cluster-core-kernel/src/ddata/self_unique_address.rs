//! Explicit self-node identity for node-local CRDT updates.

#[cfg(test)]
#[path = "self_unique_address_test.rs"]
mod tests;

use alloc::string::ToString;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

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

  /// Builds the local-node identity from a cluster authority string such as `host:port`.
  #[must_use]
  pub fn from_authority(authority: &str) -> Self {
    Self::new(unique_address_from_authority(authority))
  }

  /// Returns the wrapped unique address.
  #[must_use]
  pub const fn unique_address(&self) -> &UniqueAddress {
    &self.unique_address
  }
}

fn unique_address_from_authority(authority: &str) -> UniqueAddress {
  let (host, port) = authority_host_port(authority);
  UniqueAddress::new(Address::new("fraktor-cluster", host, port), 1)
}

fn authority_host_port(authority: &str) -> (alloc::string::String, u16) {
  if let Some((host, port_text)) = authority.rsplit_once(':')
    && let Ok(port) = port_text.parse::<u16>()
  {
    (host.to_string(), port)
  } else {
    (authority.to_string(), 0)
  }
}
