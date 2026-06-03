//! Owner-local pub-sub registry bucket.

#[cfg(test)]
#[path = "topic_registry_bucket_test.rs"]
mod tests;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  MediatorPathKey, PubSubSubscriber, PubSubTopic, TopicRegistryBucketView, TopicRegistryEntry, TopicRegistryEntryKey,
  TopicRegistryEntryKind, TopicRegistryStatus, TopicRegistryVersion,
};

/// Topic and path registry entries owned by one cluster node incarnation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRegistryBucket {
  owner:                  UniqueAddress,
  version:                TopicRegistryVersion,
  entries:                BTreeMap<TopicRegistryEntryKey, TopicRegistryEntry>,
  observed_version_floor: TopicRegistryVersion,
}

impl TopicRegistryBucket {
  /// Creates an empty registry bucket for the owner.
  #[must_use]
  pub const fn new(owner: UniqueAddress) -> Self {
    Self {
      owner,
      version: TopicRegistryVersion::zero(),
      entries: BTreeMap::new(),
      observed_version_floor: TopicRegistryVersion::zero(),
    }
  }

  /// Returns the bucket owner.
  #[must_use]
  pub const fn owner(&self) -> &UniqueAddress {
    &self.owner
  }

  /// Returns the current bucket version.
  #[must_use]
  pub const fn version(&self) -> TopicRegistryVersion {
    self.version
  }

  /// Returns the highest owner version represented by the bucket state for gossip status.
  #[must_use]
  pub fn status_version(&self) -> TopicRegistryVersion {
    let mut version = self.version;
    if version < self.observed_version_floor {
      version = self.observed_version_floor;
    }
    for entry in self.entries.values() {
      if version < entry.version() {
        version = entry.version();
      }
    }
    version
  }

  /// Returns an entry by key.
  #[must_use]
  pub fn entry(&self, key: &TopicRegistryEntryKey) -> Option<&TopicRegistryEntry> {
    self.entries.get(key)
  }

  /// Returns all entries in deterministic key order.
  #[must_use]
  pub fn entries(&self) -> Vec<TopicRegistryEntry> {
    self.entries.values().cloned().collect()
  }

  /// Returns all entries with keys in deterministic key order.
  #[must_use]
  pub fn entries_with_keys(&self) -> Vec<(TopicRegistryEntryKey, TopicRegistryEntry)> {
    self.entries.iter().map(|(key, entry)| (key.clone(), entry.clone())).collect()
  }

  /// Returns true when a remote entry is newer than the current local entry.
  #[must_use]
  pub fn should_apply_remote_entry(&self, key: &TopicRegistryEntryKey, entry: &TopicRegistryEntry) -> bool {
    match self.entries.get(key) {
      | Some(current) => current.version() < entry.version(),
      | None => self.observed_version_floor < entry.version(),
    }
  }

  /// Applies a remote entry after the caller has checked freshness and owner activity.
  pub fn apply_remote_entry(&mut self, key: TopicRegistryEntryKey, entry: TopicRegistryEntry) {
    self.apply_remote_entry_with_observed_version_floor(key, entry, TopicRegistryVersion::zero());
  }

  /// Applies a remote entry with the highest owner version the delta sender already observed before
  /// this entry.
  pub fn apply_remote_entry_with_observed_version_floor(
    &mut self,
    key: TopicRegistryEntryKey,
    entry: TopicRegistryEntry,
    observed_version_floor: TopicRegistryVersion,
  ) {
    if self.observed_version_floor < observed_version_floor {
      self.observed_version_floor = observed_version_floor;
    }
    self.entries.insert(key, entry);
    self.advance_observed_version();
  }

  /// Rebinds the bucket owner while preserving registry versions and tombstone watermarks.
  #[must_use]
  pub fn rebind_owner(&self, owner: UniqueAddress) -> Self {
    Self {
      owner,
      version: self.version,
      entries: self.entries.clone(),
      observed_version_floor: self.observed_version_floor,
    }
  }

  /// Adds or replaces a path registration.
  pub fn put_path(&mut self, path: MediatorPathKey, target: PubSubSubscriber) {
    let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };
    let kind = TopicRegistryEntryKind::Path { path, target };
    self.insert_entry(key, kind);
  }

  /// Adds or replaces a topic subscription registration.
  pub fn put_subscription(&mut self, topic: PubSubTopic, group: Option<String>, subscriber: PubSubSubscriber) {
    let key = TopicRegistryEntryKey::TopicSubscription {
      topic:      topic.clone(),
      group:      group.clone(),
      subscriber: subscriber.clone(),
    };
    let kind = TopicRegistryEntryKind::TopicSubscription { topic, group, subscriber };
    self.insert_entry(key, kind);
  }

  /// Replaces a path registration with a removed tombstone.
  pub fn remove_path(&mut self, path: MediatorPathKey, target: PubSubSubscriber, removed_at_millis: u64) {
    let key = TopicRegistryEntryKey::Path { path, target };
    self.insert_entry(key, TopicRegistryEntryKind::Removed { removed_at_millis });
  }

  /// Replaces a topic subscription with a removed tombstone.
  pub fn remove_subscription(
    &mut self,
    topic: PubSubTopic,
    group: Option<String>,
    subscriber: PubSubSubscriber,
    removed_at_millis: u64,
  ) {
    let key = TopicRegistryEntryKey::TopicSubscription { topic, group, subscriber };
    self.insert_entry(key, TopicRegistryEntryKind::Removed { removed_at_millis });
  }

  /// Prunes removed tombstones whose retention TTL has elapsed.
  pub fn prune_removed(&mut self, now_millis: u64, removed_entry_ttl: Duration, peer_statuses: &[TopicRegistryStatus]) {
    let mut pruned = Vec::new();
    self.entries.retain(|key, entry| {
      let TopicRegistryEntryKind::Removed { removed_at_millis } = entry.kind() else {
        return true;
      };
      let age_millis = now_millis.saturating_sub(*removed_at_millis);
      let ttl_elapsed = u128::from(age_millis) >= removed_entry_ttl.as_millis();
      let observed_by_peers = peer_statuses.iter().all(|status| status.version_for(&self.owner) >= entry.version());
      if ttl_elapsed && observed_by_peers {
        pruned.push((key.clone(), entry.version()));
        return false;
      }
      true
    });
    if !pruned.is_empty() {
      for (_key, version) in pruned {
        if self.observed_version_floor < version {
          self.observed_version_floor = version;
        }
      }
    }
  }

  /// Creates a delivery view that excludes entries when this owner is no longer active.
  #[must_use]
  pub fn delivery_view(&self, active_owners: &[UniqueAddress]) -> TopicRegistryBucketView {
    let delivery_candidate = active_owners.iter().any(|owner| owner == &self.owner);
    let entries = if delivery_candidate {
      self.entries.values().filter(|entry| !entry.kind().is_removed()).cloned().collect()
    } else {
      Vec::new()
    };
    TopicRegistryBucketView::new(self.owner.clone(), self.version, delivery_candidate, entries)
  }

  fn insert_entry(&mut self, key: TopicRegistryEntryKey, kind: TopicRegistryEntryKind) {
    self.version = self.version.next();
    if let Some(previous) = self.entries.insert(key, TopicRegistryEntry::new(self.version, kind))
      && self.observed_version_floor < previous.version()
    {
      self.observed_version_floor = previous.version();
    }
  }

  fn advance_observed_version(&mut self) {
    loop {
      let next_version = self.version.next();
      if !self.has_observed_version(next_version) {
        break;
      }
      self.version = next_version;
    }
  }

  fn has_observed_version(&self, version: TopicRegistryVersion) -> bool {
    version <= self.observed_version_floor || self.entries.values().any(|entry| entry.version() == version)
  }
}
