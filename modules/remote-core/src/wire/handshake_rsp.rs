//! Handshake response body.

use crate::address::UniqueAddress;

/// Body of a handshake response carrying the origin node identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandshakeRsp {
  from: UniqueAddress,
}

impl HandshakeRsp {
  /// Creates a new [`HandshakeRsp`].
  #[must_use]
  pub const fn new(from: UniqueAddress) -> Self {
    Self { from }
  }

  /// Returns the unique address of the sender.
  #[must_use]
  pub const fn from(&self) -> &UniqueAddress {
    &self.from
  }
}
