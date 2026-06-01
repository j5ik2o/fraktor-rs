//! Peer seen-version digest for gossip convergence.

use alloc::{collections::BTreeMap, vec::Vec};

use fraktor_remote_core_rs::address::UniqueAddress;

use super::MembershipVersion;

/// Tracks the latest membership version observed by each peer identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipSeenDigest {
  seen_versions: BTreeMap<UniqueAddress, MembershipVersion>,
}

impl GossipSeenDigest {
  /// Creates an empty seen digest.
  #[must_use]
  pub const fn new() -> Self {
    Self { seen_versions: BTreeMap::new() }
  }

  /// Records that `peer` has observed `version`.
  ///
  /// Returns true when the digest changed.
  pub fn mark_seen(&mut self, peer: UniqueAddress, version: MembershipVersion) -> bool {
    let entry = self.seen_versions.entry(peer).or_insert(MembershipVersion::zero());
    if *entry >= version {
      return false;
    }
    *entry = version;
    true
  }

  /// Returns the observed version for a peer.
  #[must_use]
  pub fn observed_version(&self, peer: &UniqueAddress) -> Option<MembershipVersion> {
    self.seen_versions.get(peer).copied()
  }

  /// Returns true when all active peers have observed at least `version`.
  #[must_use]
  pub fn has_seen_all(&self, active_peers: &[UniqueAddress], version: MembershipVersion) -> bool {
    active_peers.iter().all(|peer| self.seen_versions.get(peer).is_some_and(|observed| *observed >= version))
  }

  /// Returns peer-version entries in deterministic peer identity order.
  #[must_use]
  pub fn entries(&self) -> Vec<(UniqueAddress, MembershipVersion)> {
    self.seen_versions.iter().map(|(peer, version)| (peer.clone(), *version)).collect()
  }

  /// Merges another digest, keeping the highest version per peer.
  pub fn merge(&mut self, other: &Self) -> bool {
    let mut changed = false;
    for (peer, version) in other.seen_versions.iter() {
      changed |= self.mark_seen(peer.clone(), *version);
    }
    changed
  }
}

impl Default for GossipSeenDigest {
  fn default() -> Self {
    Self::new()
  }
}
