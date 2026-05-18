#[cfg(test)]
#[path = "remote_scope_test.rs"]
mod tests;

use crate::actor::Address;

/// Deployment scope targeting a remote actor system address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteScope {
  node: Address,
}

impl RemoteScope {
  /// Creates a remote deployment scope for the target node.
  ///
  /// # Panics
  ///
  /// Panics when `node` is a local address (no host/port). `RemoteScope` is a
  /// remote-only deployment marker and a local address would silently violate
  /// that contract.
  #[must_use]
  pub fn new(node: Address) -> Self {
    assert!(node.has_global_scope(), "RemoteScope requires a remote address with host and port");
    Self { node }
  }

  /// Returns the target remote node.
  #[must_use]
  pub const fn node(&self) -> &Address {
    &self.node
  }
}
