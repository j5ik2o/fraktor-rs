//! Delivery candidate view over a pub-sub registry bucket.

use alloc::vec::Vec;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{TopicRegistryEntry, TopicRegistryVersion};

/// Snapshot view used by delivery selection to ignore removed owners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRegistryBucketView {
  owner:              UniqueAddress,
  version:            TopicRegistryVersion,
  delivery_candidate: bool,
  entries:            Vec<TopicRegistryEntry>,
}

impl TopicRegistryBucketView {
  /// Creates a bucket view.
  #[must_use]
  pub const fn new(
    owner: UniqueAddress,
    version: TopicRegistryVersion,
    delivery_candidate: bool,
    entries: Vec<TopicRegistryEntry>,
  ) -> Self {
    Self { owner, version, delivery_candidate, entries }
  }

  /// Returns the bucket owner.
  #[must_use]
  pub const fn owner(&self) -> &UniqueAddress {
    &self.owner
  }

  /// Returns the bucket version.
  #[must_use]
  pub const fn version(&self) -> TopicRegistryVersion {
    self.version
  }

  /// Returns true when entries from this bucket may be used for delivery.
  #[must_use]
  pub const fn is_delivery_candidate(&self) -> bool {
    self.delivery_candidate
  }

  /// Returns non-removed entries when this bucket is a delivery candidate.
  #[must_use]
  pub fn entries(&self) -> &[TopicRegistryEntry] {
    &self.entries
  }
}
