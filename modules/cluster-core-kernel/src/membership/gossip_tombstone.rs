//! Removed or dead member tombstone.

use fraktor_remote_core_rs::address::UniqueAddress;

use super::MembershipVersion;

/// Tombstone that prevents stale member records from reappearing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipTombstone {
  /// Member identity covered by the tombstone.
  pub member:  UniqueAddress,
  /// Membership version that produced the removed or dead record.
  pub version: MembershipVersion,
}

impl GossipTombstone {
  /// Creates a tombstone for a member identity and version.
  #[must_use]
  pub const fn new(member: UniqueAddress, version: MembershipVersion) -> Self {
    Self { member, version }
  }
}
