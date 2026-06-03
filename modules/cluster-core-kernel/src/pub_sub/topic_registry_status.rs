//! Pub-sub registry owner version status.

use alloc::vec::Vec;

use fraktor_remote_core_rs::address::UniqueAddress;

use crate::pub_sub::{TopicRegistryBucket, TopicRegistryVersion};

/// Owner-version map exchanged before pub-sub registry deltas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRegistryStatus {
  owner_versions: Vec<(UniqueAddress, TopicRegistryVersion)>,
}

impl TopicRegistryStatus {
  /// Creates a deterministic owner-version status.
  #[must_use]
  pub fn new(mut owner_versions: Vec<(UniqueAddress, TopicRegistryVersion)>) -> Self {
    owner_versions.sort_by(|left, right| left.0.cmp(&right.0));
    let mut deduplicated = Vec::new();
    for (owner, version) in owner_versions {
      if let Some((last_owner, last_version)) = deduplicated.last_mut()
        && last_owner == &owner
      {
        if *last_version < version {
          *last_version = version;
        }
        continue;
      }
      deduplicated.push((owner, version));
    }
    Self { owner_versions: deduplicated }
  }

  /// Creates status from local buckets.
  #[must_use]
  pub fn from_buckets(buckets: &[TopicRegistryBucket]) -> Self {
    Self::new(buckets.iter().map(|bucket| (bucket.owner().clone(), bucket.status_version())).collect())
  }

  /// Returns the version known for one owner.
  #[must_use]
  pub fn version_for(&self, owner: &UniqueAddress) -> TopicRegistryVersion {
    self
      .owner_versions
      .iter()
      .find(|(candidate, _)| candidate == owner)
      .map(|(_, version)| *version)
      .unwrap_or_default()
  }

  /// Returns owner versions in deterministic owner order.
  #[must_use]
  pub fn owner_versions(&self) -> &[(UniqueAddress, TopicRegistryVersion)] {
    &self.owner_versions
  }
}

impl Default for TopicRegistryStatus {
  fn default() -> Self {
    Self::new(Vec::new())
  }
}
