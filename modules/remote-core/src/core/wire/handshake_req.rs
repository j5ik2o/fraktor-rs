//! Handshake request body.

use crate::core::address::{Address, UniqueAddress};

/// Body of a handshake request carrying the origin node identity and destination address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandshakeReq {
  from: UniqueAddress,
  to:   Address,
}

impl HandshakeReq {
  /// Creates a new [`HandshakeReq`].
  #[must_use]
  pub const fn new(from: UniqueAddress, to: Address) -> Self {
    Self { from, to }
  }

  /// Returns the unique address of the sender.
  #[must_use]
  pub const fn from(&self) -> &UniqueAddress {
    &self.from
  }

  /// Returns the expected local destination address.
  #[must_use]
  pub const fn to(&self) -> &Address {
    &self.to
  }
}
