//! Versioned registry entry carried in a pub-sub delta.

use fraktor_remote_core_rs::address::UniqueAddress;

use crate::pub_sub::{TopicRegistryEntry, TopicRegistryEntryKey, TopicRegistryVersion};

/// One owner-scoped registry entry included in a delta payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRegistryDeltaEntry {
  owner:                  UniqueAddress,
  key:                    TopicRegistryEntryKey,
  entry:                  TopicRegistryEntry,
  observed_version_floor: TopicRegistryVersion,
}

impl TopicRegistryDeltaEntry {
  /// Creates a delta entry.
  #[must_use]
  pub const fn new(owner: UniqueAddress, key: TopicRegistryEntryKey, entry: TopicRegistryEntry) -> Self {
    Self { owner, key, entry, observed_version_floor: TopicRegistryVersion::zero() }
  }

  /// Creates a delta entry with a compact owner-version watermark observed before this entry.
  #[must_use]
  pub const fn new_with_observed_version_floor(
    owner: UniqueAddress,
    key: TopicRegistryEntryKey,
    entry: TopicRegistryEntry,
    observed_version_floor: TopicRegistryVersion,
  ) -> Self {
    Self { owner, key, entry, observed_version_floor }
  }

  /// Returns the owner bucket identity.
  #[must_use]
  pub const fn owner(&self) -> &UniqueAddress {
    &self.owner
  }

  /// Returns the registry key.
  #[must_use]
  pub const fn key(&self) -> &TopicRegistryEntryKey {
    &self.key
  }

  /// Returns the versioned registry entry.
  #[must_use]
  pub const fn entry(&self) -> &TopicRegistryEntry {
    &self.entry
  }

  /// Returns the highest contiguous owner version observed before this entry.
  #[must_use]
  pub const fn observed_version_floor(&self) -> TopicRegistryVersion {
    self.observed_version_floor
  }
}
