//! Uniquely identified remote address (`Address` + monotonic UID).

use core::{
  fmt,
  hash::{Hash, Hasher},
};

use crate::address::Address;

/// Remote address identity including a monotonic unique identifier (`uid`).
///
/// The `uid` is a `u64` per design Decision 13 (Pekko's `Long` analogue); `0` is
/// reserved as an "unconfirmed" sentinel (e.g. before the handshake has completed).
#[derive(Clone, Debug)]
pub struct UniqueAddress {
  address: Address,
  uid:     u64,
}

impl UniqueAddress {
  /// Creates a new [`UniqueAddress`].
  #[must_use]
  pub const fn new(address: Address, uid: u64) -> Self {
    Self { address, uid }
  }

  /// Returns the underlying [`Address`].
  #[must_use]
  pub const fn address(&self) -> &Address {
    &self.address
  }

  /// Returns the unique identifier. `0` is the sentinel for "unconfirmed".
  #[must_use]
  pub const fn uid(&self) -> u64 {
    self.uid
  }
}

impl PartialEq for UniqueAddress {
  fn eq(&self, other: &Self) -> bool {
    self.uid == other.uid && self.address == other.address
  }
}

impl Eq for UniqueAddress {}

impl Hash for UniqueAddress {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.address.hash(state);
    self.uid.hash(state);
  }
}

impl fmt::Display for UniqueAddress {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}#{}", self.address, self.uid)
  }
}
