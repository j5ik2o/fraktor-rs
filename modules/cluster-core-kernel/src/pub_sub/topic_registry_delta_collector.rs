//! Pub-sub registry status comparison and delta application.

#[cfg(test)]
#[path = "topic_registry_delta_collector_test.rs"]
mod tests;

use alloc::vec::Vec;

use fraktor_remote_core_rs::address::UniqueAddress;

use crate::pub_sub::{
  DistributedPubSubConfig, TopicRegistryApplyOutcome, TopicRegistryBucket, TopicRegistryDelta, TopicRegistryDeltaEntry,
  TopicRegistryStatus, TopicRegistryVersion,
};

/// Collects and applies bounded pub-sub registry deltas.
pub struct TopicRegistryDeltaCollector;

impl TopicRegistryDeltaCollector {
  /// Collects local entries newer than `peer_status`, bounded by settings.
  #[must_use]
  pub fn collect_delta(
    peer_status: &TopicRegistryStatus,
    local_buckets: &[TopicRegistryBucket],
    settings: &DistributedPubSubConfig,
  ) -> TopicRegistryDelta {
    let mut entries = Vec::new();
    for bucket in local_buckets {
      let peer_version = peer_status.version_for(bucket.owner());
      for (key, entry) in bucket.entries_with_keys() {
        if entry.version() > peer_version {
          let observed_version_floor = observed_version_floor(peer_version, entry.version());
          entries.push(TopicRegistryDeltaEntry::new_with_observed_version_floor(
            bucket.owner().clone(),
            key,
            entry,
            observed_version_floor,
          ));
        }
      }
    }
    entries.sort_by(|left, right| {
      left
        .entry()
        .version()
        .cmp(&right.entry().version())
        .then(left.owner().cmp(right.owner()))
        .then(left.key().cmp(right.key()))
    });
    entries.truncate(settings.max_delta_elements());
    TopicRegistryDelta::new(entries)
  }

  /// Applies a delta to existing owner buckets and reports ignored entries.
  #[must_use]
  pub fn apply_delta(
    delta: &TopicRegistryDelta,
    local_buckets: &mut Vec<TopicRegistryBucket>,
    active_owners: &[UniqueAddress],
  ) -> Vec<TopicRegistryApplyOutcome> {
    let mut outcomes = Vec::new();
    for delta_entry in delta.entries() {
      if !active_owners.iter().any(|owner| owner == delta_entry.owner()) {
        outcomes.push(TopicRegistryApplyOutcome::IgnoredInactiveOwner { owner: delta_entry.owner().clone() });
        continue;
      }

      let bucket_index =
        if let Some(index) = local_buckets.iter().position(|bucket| bucket.owner() == delta_entry.owner()) {
          index
        } else {
          local_buckets.push(TopicRegistryBucket::new(delta_entry.owner().clone()));
          local_buckets.len() - 1
        };
      let bucket = &mut local_buckets[bucket_index];

      if !bucket.should_apply_remote_entry(delta_entry.key(), delta_entry.entry()) {
        outcomes.push(TopicRegistryApplyOutcome::IgnoredStale {
          owner:   delta_entry.owner().clone(),
          version: delta_entry.entry().version(),
        });
        continue;
      }

      bucket.apply_remote_entry_with_observed_version_floor(
        delta_entry.key().clone(),
        delta_entry.entry().clone(),
        delta_entry.observed_version_floor(),
      );
      outcomes.push(TopicRegistryApplyOutcome::Applied {
        owner:   delta_entry.owner().clone(),
        version: delta_entry.entry().version(),
      });
    }
    outcomes
  }
}

const fn observed_version_floor(
  peer_version: TopicRegistryVersion,
  entry_version: TopicRegistryVersion,
) -> TopicRegistryVersion {
  if entry_version.value() <= peer_version.value().saturating_add(1) {
    peer_version
  } else {
    TopicRegistryVersion::new(entry_version.value().saturating_sub(1))
  }
}
