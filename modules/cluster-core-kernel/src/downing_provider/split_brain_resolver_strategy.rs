//! Split Brain Resolver active strategy identifiers.

const KEEP_MAJORITY_IDENTIFIER: &str = "keep-majority";
const LEASE_MAJORITY_IDENTIFIER: &str = "lease-majority";
const STATIC_QUORUM_IDENTIFIER: &str = "static-quorum";
const KEEP_OLDEST_IDENTIFIER: &str = "keep-oldest";
const DOWN_ALL_IDENTIFIER: &str = "down-all";

/// Split Brain Resolver strategy selected as the active downing rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SplitBrainResolverStrategy {
  /// Keep the partition that contains a majority of observed members.
  KeepMajority,
  /// Keep the majority partition only after acquiring a lease.
  LeaseMajority,
  /// Keep the partition that satisfies a configured static quorum.
  StaticQuorum,
  /// Keep the partition that contains the oldest member.
  KeepOldest,
  /// Down every observed member when the cluster remains unstable.
  DownAll,
}

impl SplitBrainResolverStrategy {
  /// Returns the Pekko-compatible strategy identifier.
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    match self {
      | Self::KeepMajority => KEEP_MAJORITY_IDENTIFIER,
      | Self::LeaseMajority => LEASE_MAJORITY_IDENTIFIER,
      | Self::StaticQuorum => STATIC_QUORUM_IDENTIFIER,
      | Self::KeepOldest => KEEP_OLDEST_IDENTIFIER,
      | Self::DownAll => DOWN_ALL_IDENTIFIER,
    }
  }
}
