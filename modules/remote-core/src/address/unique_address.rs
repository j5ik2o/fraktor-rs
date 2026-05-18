//! Uniquely identified remote address (`Address` + monotonic UID).

use core::fmt::{Display, Formatter, Result as FmtResult};

use crate::address::Address;

/// Remote address identity including a monotonic unique identifier (`uid`).
///
/// The `uid` is a `u64` per design Decision 13 (Pekko's `Long` analogue); `0` is
/// reserved as an "unconfirmed" sentinel (e.g. before the handshake has completed).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

impl Display for UniqueAddress {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "{}#{}", self.address, self.uid)
  }
}
