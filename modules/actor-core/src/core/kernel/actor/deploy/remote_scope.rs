#[cfg(test)]
mod tests;

use crate::core::kernel::actor::Address;

/// Deployment scope targeting a remote actor system address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteScope {
  node: Address,
}

impl RemoteScope {
  /// Creates a remote deployment scope for the target node.
  #[must_use]
  pub const fn new(node: Address) -> Self {
    Self { node }
  }

  /// Returns the target remote node.
  #[must_use]
  pub const fn node(&self) -> &Address {
    &self.node
  }
}
