use alloc::vec;
use core::{slice::from_ref, time::Duration};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::{
  activation::ClusterIdentity,
  pub_sub::{
    DistributedPubSubConfig, MediatorPathKey, PubSubNoSubscriberBehavior, PubSubRoutingMode, PubSubSubscriber,
    PubSubTopic, TopicRegistryApplyOutcome, TopicRegistryBucket, TopicRegistryDelta, TopicRegistryDeltaCollector,
    TopicRegistryDeltaEntry, TopicRegistryEntry, TopicRegistryEntryKey, TopicRegistryEntryKind, TopicRegistryStatus,
    TopicRegistryVersion,
  },
};

fn owner(name: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", name, 2552), uid)
}

fn subscriber(name: &str) -> PubSubSubscriber {
  PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", name).expect("identity"))
}

fn path(value: &str) -> MediatorPathKey {
  MediatorPathKey::parse(value).expect("path")
}

fn config(max_delta_elements: usize) -> DistributedPubSubConfig {
  DistributedPubSubConfig::try_new(
    None,
    PubSubRoutingMode::Random,
    Duration::from_secs(1),
    Duration::from_millis(10),
    max_delta_elements,
    PubSubNoSubscriberBehavior::Drop,
  )
  .expect("config")
}

#[test]
fn status_reports_owner_bucket_versions() {
  let first_owner = owner("node-a", 1);
  let second_owner = owner("node-b", 2);
  let mut first = TopicRegistryBucket::new(first_owner.clone());
  let second = TopicRegistryBucket::new(second_owner.clone());
  first.put_path(path("fraktor://sys/user/service"), subscriber("actor-1"));

  let status = TopicRegistryStatus::from_buckets(&[second, first]);

  assert_eq!(status.version_for(&first_owner), TopicRegistryVersion::new(1));
  assert_eq!(status.version_for(&second_owner), TopicRegistryVersion::zero());
  assert_eq!(status.owner_versions()[0].0, first_owner);
}

#[test]
fn status_uses_highest_version_for_duplicate_owner_entries() {
  let first_owner = owner("node-a", 1);
  let second_owner = owner("node-b", 2);

  let status = TopicRegistryStatus::new(vec![
    (first_owner.clone(), TopicRegistryVersion::new(1)),
    (second_owner.clone(), TopicRegistryVersion::new(3)),
    (first_owner.clone(), TopicRegistryVersion::new(5)),
    (second_owner.clone(), TopicRegistryVersion::new(2)),
  ]);

  assert_eq!(status.version_for(&first_owner), TopicRegistryVersion::new(5));
  assert_eq!(status.version_for(&second_owner), TopicRegistryVersion::new(3));
  assert_eq!(status.owner_versions().len(), 2);
}

#[test]
fn status_from_mirror_bucket_reports_highest_entry_version() {
  let active_owner = owner("node-a", 1);
  let path = path("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };
  let mut bucket = TopicRegistryBucket::new(active_owner.clone());
  bucket.apply_remote_entry(
    key,
    TopicRegistryEntry::new(TopicRegistryVersion::new(7), TopicRegistryEntryKind::Path { path, target }),
  );

  let status = TopicRegistryStatus::from_buckets(&[bucket]);

  assert_eq!(status.version_for(&active_owner), TopicRegistryVersion::new(7));
}

#[test]
fn collector_returns_version_ordered_bounded_delta() {
  let local_owner = owner("node-a", 1);
  let mut bucket = TopicRegistryBucket::new(local_owner.clone());
  bucket.put_path(path("fraktor://sys/user/a"), subscriber("actor-a"));
  bucket.put_path(path("fraktor://sys/user/b"), subscriber("actor-b"));
  bucket.put_path(path("fraktor://sys/user/c"), subscriber("actor-c"));
  let peer_status = TopicRegistryStatus::new(vec![(local_owner, TopicRegistryVersion::new(1))]);

  let delta = TopicRegistryDeltaCollector::collect_delta(&peer_status, &[bucket], &config(1));

  assert_eq!(delta.len(), 1);
  assert_eq!(delta.entries()[0].entry().version(), TopicRegistryVersion::new(2));
}

#[test]
fn apply_delta_reports_applied_stale_new_bucket_and_inactive_outcomes() {
  let active_owner = owner("node-a", 1);
  let new_owner = owner("node-b", 2);
  let inactive_owner = owner("node-c", 3);
  let topic = PubSubTopic::new("news");
  let key = TopicRegistryEntryKey::TopicSubscription {
    topic:      topic.clone(),
    group:      None,
    subscriber: subscriber("sub-1"),
  };
  let newer = TopicRegistryEntry::new(TopicRegistryVersion::new(2), TopicRegistryEntryKind::TopicSubscription {
    topic,
    group: None,
    subscriber: subscriber("sub-1"),
  });
  let stale =
    TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::Removed { removed_at_millis: 1 });
  let mut bucket = TopicRegistryBucket::new(active_owner.clone());
  bucket.apply_remote_entry(key.clone(), newer.clone());
  let delta = TopicRegistryDelta::new(vec![
    TopicRegistryDeltaEntry::new(active_owner.clone(), key.clone(), stale),
    TopicRegistryDeltaEntry::new(new_owner.clone(), key.clone(), newer.clone()),
    TopicRegistryDeltaEntry::new(inactive_owner.clone(), key.clone(), newer.clone()),
    TopicRegistryDeltaEntry::new(
      active_owner.clone(),
      key.clone(),
      TopicRegistryEntry::new(TopicRegistryVersion::new(3), TopicRegistryEntryKind::Removed { removed_at_millis: 7 }),
    ),
  ]);
  let mut buckets = vec![bucket];

  let outcomes =
    TopicRegistryDeltaCollector::apply_delta(&delta, &mut buckets, &[active_owner.clone(), new_owner.clone()]);

  assert!(outcomes.contains(&TopicRegistryApplyOutcome::IgnoredStale {
    owner:   active_owner.clone(),
    version: TopicRegistryVersion::new(1),
  }));
  assert!(outcomes.contains(&TopicRegistryApplyOutcome::Applied {
    owner:   new_owner.clone(),
    version: TopicRegistryVersion::new(2),
  }));
  let new_bucket = buckets.iter().find(|bucket| bucket.owner() == &new_owner).expect("new owner bucket");
  assert_eq!(new_bucket.version(), TopicRegistryVersion::new(0));
  assert_eq!(new_bucket.entry(&key).expect("new owner entry").version(), TopicRegistryVersion::new(2));
  assert!(outcomes.contains(&TopicRegistryApplyOutcome::IgnoredInactiveOwner { owner: inactive_owner }));
  assert!(
    outcomes
      .contains(&TopicRegistryApplyOutcome::Applied { owner: active_owner, version: TopicRegistryVersion::new(3) })
  );
  assert!(buckets.iter().any(|bucket| bucket.owner() == &new_owner));
}

#[test]
fn tombstone_from_delta_can_be_pruned_after_ttl() {
  let active_owner = owner("node-a", 1);
  let key = TopicRegistryEntryKey::Path { path: path("fraktor://sys/user/service"), target: subscriber("actor-1") };
  let tombstone =
    TopicRegistryEntry::new(TopicRegistryVersion::new(5), TopicRegistryEntryKind::Removed { removed_at_millis: 10 });
  let delta = TopicRegistryDelta::new(vec![TopicRegistryDeltaEntry::new(active_owner.clone(), key.clone(), tombstone)]);
  let bucket = TopicRegistryBucket::new(active_owner.clone());
  let mut buckets = vec![bucket];

  let outcomes = TopicRegistryDeltaCollector::apply_delta(&delta, &mut buckets, from_ref(&active_owner));
  let bucket = buckets.first_mut().expect("bucket");

  assert!(matches!(outcomes.as_slice(), [TopicRegistryApplyOutcome::Applied { .. }]));
  assert!(matches!(bucket.entry(&key).expect("tombstone").kind(), TopicRegistryEntryKind::Removed { .. }));
  bucket.prune_removed(20, Duration::from_millis(10), &[]);
  assert!(bucket.entry(&key).is_none());
}

#[test]
fn delta_for_replaced_key_carries_observed_version_floor_for_owner_convergence() {
  let active_owner = owner("node-a", 1);
  let path = path("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };
  let mut source = TopicRegistryBucket::new(active_owner.clone());
  source.put_path(path, target.clone());
  source.remove_path(MediatorPathKey::parse("fraktor://sys/user/service").expect("path"), target, 10);
  let peer_status = TopicRegistryStatus::new(vec![(active_owner.clone(), TopicRegistryVersion::zero())]);

  let delta = TopicRegistryDeltaCollector::collect_delta(&peer_status, &[source], &config(10));
  let mut buckets = vec![TopicRegistryBucket::new(active_owner.clone())];
  let outcomes = TopicRegistryDeltaCollector::apply_delta(&delta, &mut buckets, from_ref(&active_owner));

  assert!(matches!(outcomes.as_slice(), [TopicRegistryApplyOutcome::Applied { .. }]));
  assert_eq!(delta.entries()[0].observed_version_floor(), TopicRegistryVersion::new(1));
  assert_eq!(buckets[0].version(), TopicRegistryVersion::new(2));
  assert!(matches!(buckets[0].entry(&key).expect("tombstone").kind(), TopicRegistryEntryKind::Removed { .. }));
}

#[test]
fn delta_for_lagging_bucket_carries_observed_version_floor_before_entry() {
  let active_owner = owner("node-a", 1);
  let path = path("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };
  let mut source = TopicRegistryBucket::new(active_owner.clone());
  source.apply_remote_entry(
    key,
    TopicRegistryEntry::new(TopicRegistryVersion::new(3), TopicRegistryEntryKind::Path { path, target }),
  );
  let peer_status = TopicRegistryStatus::new(vec![(active_owner.clone(), TopicRegistryVersion::zero())]);

  let delta = TopicRegistryDeltaCollector::collect_delta(&peer_status, &[source], &config(10));
  let mut buckets = vec![TopicRegistryBucket::new(active_owner.clone())];
  let outcomes = TopicRegistryDeltaCollector::apply_delta(&delta, &mut buckets, from_ref(&active_owner));

  assert_eq!(delta.entries()[0].observed_version_floor(), TopicRegistryVersion::new(2));
  assert!(matches!(outcomes.as_slice(), [TopicRegistryApplyOutcome::Applied { .. }]));
  assert_eq!(buckets[0].version(), TopicRegistryVersion::new(3));
}

#[test]
fn delta_for_large_lagging_bucket_uses_single_observed_version_floor() {
  let active_owner = owner("node-a", 1);
  let path = path("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };
  let mut source = TopicRegistryBucket::new(active_owner.clone());
  source.apply_remote_entry(
    key,
    TopicRegistryEntry::new(TopicRegistryVersion::new(200_000), TopicRegistryEntryKind::Path { path, target }),
  );
  let peer_status = TopicRegistryStatus::new(vec![(active_owner, TopicRegistryVersion::zero())]);

  let delta = TopicRegistryDeltaCollector::collect_delta(&peer_status, &[source], &config(10));

  assert_eq!(delta.len(), 1);
  assert_eq!(delta.entries()[0].observed_version_floor(), TopicRegistryVersion::new(199_999));
}

#[test]
fn replaced_key_tombstone_from_delta_can_be_pruned_after_peer_status_round_trip() {
  let active_owner = owner("node-a", 1);
  let path = path("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };
  let mut source = TopicRegistryBucket::new(active_owner.clone());
  source.put_path(path.clone(), target.clone());
  source.remove_path(path, target, 10);
  let peer_status = TopicRegistryStatus::new(vec![(active_owner.clone(), TopicRegistryVersion::zero())]);
  let delta = TopicRegistryDeltaCollector::collect_delta(&peer_status, &[source.clone()], &config(10));
  let mut peer_buckets = vec![TopicRegistryBucket::new(active_owner.clone())];

  let outcomes = TopicRegistryDeltaCollector::apply_delta(&delta, &mut peer_buckets, from_ref(&active_owner));
  let converged_peer_status = TopicRegistryStatus::from_buckets(&peer_buckets);
  source.prune_removed(20, Duration::from_millis(10), &[converged_peer_status]);

  assert!(matches!(outcomes.as_slice(), [TopicRegistryApplyOutcome::Applied { .. }]));
  assert!(source.entry(&key).is_none());
}
