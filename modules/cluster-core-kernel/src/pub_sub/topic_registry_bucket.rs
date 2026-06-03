//! Owner-local pub-sub registry bucket.

#[cfg(test)]
#[path = "topic_registry_bucket_test.rs"]
mod tests;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::UniqueAddress;

use super::{
  MediatorPathKey, PubSubSubscriber, PubSubTopic, TopicRegistryBucketView, TopicRegistryEntry, TopicRegistryEntryKey,
  TopicRegistryEntryKind, TopicRegistryVersion,
};

/// Topic and path registry entries owned by one cluster node incarnation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRegistryBucket {
  owner:   UniqueAddress,
  version: TopicRegistryVersion,
  entries: BTreeMap<TopicRegistryEntryKey, TopicRegistryEntry>,
}

impl TopicRegistryBucket {
  /// Creates an empty registry bucket for the owner.
  #[must_use]
  pub const fn new(owner: UniqueAddress) -> Self {
    Self { owner, version: TopicRegistryVersion::zero(), entries: BTreeMap::new() }
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

  /// Adds or replaces a path registration.
  pub fn put_path(&mut self, path: MediatorPathKey, target: PubSubSubscriber) -> TopicRegistryVersion {
    let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };
    let kind = TopicRegistryEntryKind::Path { path, target };
    self.insert_entry(key, kind)
  }

  /// Adds or replaces a topic subscription registration.
  pub fn put_subscription(
    &mut self,
    topic: PubSubTopic,
    group: Option<String>,
    subscriber: PubSubSubscriber,
  ) -> TopicRegistryVersion {
    let key = TopicRegistryEntryKey::TopicSubscription {
      topic:      topic.clone(),
      group:      group.clone(),
      subscriber: subscriber.clone(),
    };
    let kind = TopicRegistryEntryKind::TopicSubscription { topic, group, subscriber };
    self.insert_entry(key, kind)
  }

  /// Replaces a path registration with a removed tombstone.
  pub fn remove_path(
    &mut self,
    path: MediatorPathKey,
    target: PubSubSubscriber,
    removed_at_millis: u64,
  ) -> TopicRegistryVersion {
    let key = TopicRegistryEntryKey::Path { path, target };
    self.insert_entry(key, TopicRegistryEntryKind::Removed { removed_at_millis })
  }

  /// Replaces a topic subscription with a removed tombstone.
  pub fn remove_subscription(
    &mut self,
    topic: PubSubTopic,
    group: Option<String>,
    subscriber: PubSubSubscriber,
    removed_at_millis: u64,
  ) -> TopicRegistryVersion {
    let key = TopicRegistryEntryKey::TopicSubscription { topic, group, subscriber };
    self.insert_entry(key, TopicRegistryEntryKind::Removed { removed_at_millis })
  }

  /// Prunes removed tombstones whose retention TTL has elapsed.
  pub fn prune_removed(&mut self, now_millis: u64, removed_entry_ttl: Duration) -> usize {
    let before = self.entries.len();
    self.entries.retain(|_, entry| {
      let TopicRegistryEntryKind::Removed { removed_at_millis } = entry.kind() else {
        return true;
      };
      let age_millis = now_millis.saturating_sub(*removed_at_millis);
      u128::from(age_millis) < removed_entry_ttl.as_millis()
    });
    let pruned = before - self.entries.len();
    if pruned > 0 {
      self.version = self.version.next();
    }
    pruned
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

  fn insert_entry(&mut self, key: TopicRegistryEntryKey, kind: TopicRegistryEntryKind) -> TopicRegistryVersion {
    self.version = self.version.next();
    self.entries.insert(key, TopicRegistryEntry::new(self.version, kind));
    self.version
  }
}
