use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::{
  activation::ClusterIdentity,
  pub_sub::{
    MediatorPathKey, PubSubSubscriber, PubSubTopic, TopicRegistryBucket, TopicRegistryEntry, TopicRegistryEntryKey,
    TopicRegistryEntryKind, TopicRegistryStatus, TopicRegistryVersion,
  },
};

fn owner(name: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", name, 2552), uid)
}

fn path_key(value: &str) -> MediatorPathKey {
  MediatorPathKey::parse(value).expect("path")
}

fn subscriber(name: &str) -> PubSubSubscriber {
  PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", name).expect("identity"))
}

#[test]
fn put_path_advances_version_and_keeps_entry() {
  let mut bucket = TopicRegistryBucket::new(owner("node-a", 1));
  let path = path_key("fraktor://sys/user/service");
  let target = subscriber("actor-1");

  bucket.put_path(path.clone(), target.clone());

  let key = TopicRegistryEntryKey::Path { path, target };
  let entry = bucket.entry(&key).expect("entry");
  assert_eq!(bucket.version(), TopicRegistryVersion::new(1));
  assert_eq!(entry.version(), TopicRegistryVersion::new(1));
  assert!(matches!(entry.kind(), TopicRegistryEntryKind::Path { .. }));
}

#[test]
fn remove_path_keeps_tombstone_until_ttl_elapses() {
  let mut bucket = TopicRegistryBucket::new(owner("node-a", 1));
  let path = path_key("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };

  bucket.put_path(path.clone(), target.clone());
  bucket.remove_path(path, target, 1_000);

  let entry = bucket.entry(&key).expect("tombstone");
  assert_eq!(bucket.version(), TopicRegistryVersion::new(2));
  assert!(matches!(entry.kind(), TopicRegistryEntryKind::Removed { removed_at_millis: 1_000 }));
  bucket.prune_removed(1_500, Duration::from_millis(1_000), &[]);
  assert!(bucket.entry(&key).is_some());
  assert_eq!(bucket.version(), TopicRegistryVersion::new(2));
  bucket.prune_removed(2_000, Duration::from_millis(1_000), &[]);
  assert!(bucket.entry(&key).is_none());
  assert_eq!(bucket.version(), TopicRegistryVersion::new(2));
}

#[test]
fn topic_subscription_and_path_entries_use_separate_namespaces() {
  let mut bucket = TopicRegistryBucket::new(owner("node-a", 1));
  let path = path_key("fraktor://sys/user/news");
  let target = subscriber("actor-1");
  let topic = PubSubTopic::new("news");
  let group = Some(String::from("blue"));
  let subscriber = subscriber("sub-1");

  bucket.put_path(path.clone(), target.clone());
  bucket.put_subscription(topic.clone(), group.clone(), subscriber.clone());

  assert!(bucket.entry(&TopicRegistryEntryKey::Path { path, target }).is_some());
  assert!(bucket.entry(&TopicRegistryEntryKey::TopicSubscription { topic, group, subscriber }).is_some());
  assert_eq!(bucket.entries().len(), 2);
}

#[test]
fn unsubscribe_keeps_topic_tombstone_and_prunes_it() {
  let mut bucket = TopicRegistryBucket::new(owner("node-a", 1));
  let topic = PubSubTopic::new("news");
  let subscriber = subscriber("sub-1");
  let key = TopicRegistryEntryKey::TopicSubscription {
    topic:      topic.clone(),
    group:      None,
    subscriber: subscriber.clone(),
  };

  bucket.put_subscription(topic.clone(), None, subscriber.clone());
  bucket.remove_subscription(topic, None, subscriber, 10);

  assert!(matches!(bucket.entry(&key).expect("tombstone").kind(), TopicRegistryEntryKind::Removed {
    removed_at_millis: 10,
  }));
  bucket.prune_removed(20, Duration::from_millis(10), &[]);
  assert!(bucket.entry(&key).is_none());
}

#[test]
fn stale_remote_entry_does_not_resurrect_pruned_tombstone() {
  let mut bucket = TopicRegistryBucket::new(owner("node-a", 1));
  let path = path_key("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };

  bucket.put_path(path.clone(), target.clone());
  let stale_entry = bucket.entry(&key).expect("registered").clone();
  bucket.remove_path(path, target, 10);
  bucket.prune_removed(20, Duration::from_millis(10), &[]);

  assert_eq!(bucket.version(), TopicRegistryVersion::new(2));
  assert!(bucket.entry(&key).is_none());
  assert!(!bucket.should_apply_remote_entry(&key, &stale_entry));
}

#[test]
fn rebind_owner_preserves_bucket_version_after_tombstone_prune() {
  let mut bucket = TopicRegistryBucket::new(owner("node-a", 1));
  let path = path_key("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };

  bucket.put_path(path.clone(), target.clone());
  let stale_entry = bucket.entry(&key).expect("registered").clone();
  bucket.remove_path(path, target, 10);
  bucket.prune_removed(20, Duration::from_millis(10), &[]);

  let rebound = bucket.rebind_owner(owner("node-a", 2));

  assert_eq!(rebound.version(), TopicRegistryVersion::new(2));
  assert!(rebound.entry(&key).is_none());
  assert!(!rebound.should_apply_remote_entry(&key, &stale_entry));
}

#[test]
fn missing_remote_entry_is_not_stale_without_pruned_tombstone_watermark() {
  let mut bucket = TopicRegistryBucket::new(owner("node-a", 1));
  let existing_path = path_key("fraktor://sys/user/existing");
  let existing_target = subscriber("actor-existing");
  bucket.put_path(existing_path, existing_target);

  let missing_path = path_key("fraktor://sys/user/missing");
  let missing_target = subscriber("actor-missing");
  let missing_key = TopicRegistryEntryKey::Path { path: missing_path.clone(), target: missing_target.clone() };
  let missing_entry = TopicRegistryEntry::new(TopicRegistryVersion::new(2), TopicRegistryEntryKind::Path {
    path:   missing_path,
    target: missing_target,
  });

  assert_eq!(bucket.version(), TopicRegistryVersion::new(1));
  assert!(bucket.entry(&missing_key).is_none());
  assert!(bucket.should_apply_remote_entry(&missing_key, &missing_entry));
}

#[test]
fn missing_remote_entry_older_than_bucket_version_is_stale() {
  let mut bucket = TopicRegistryBucket::new(owner("node-a", 1));
  let current_path = path_key("fraktor://sys/user/current");
  let current_target = subscriber("actor-current");
  let current_key = TopicRegistryEntryKey::Path { path: current_path.clone(), target: current_target.clone() };
  let current_entry = TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::Path {
    path:   current_path,
    target: current_target,
  });
  let stale_path = path_key("fraktor://sys/user/stale");
  let stale_target = subscriber("actor-stale");
  let stale_key = TopicRegistryEntryKey::Path { path: stale_path.clone(), target: stale_target.clone() };
  let stale_entry = TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::Path {
    path:   stale_path,
    target: stale_target,
  });

  bucket.apply_remote_entry(current_key, current_entry);

  assert_eq!(bucket.version(), TopicRegistryVersion::new(1));
  assert!(bucket.entry(&stale_key).is_none());
  assert!(!bucket.should_apply_remote_entry(&stale_key, &stale_entry));
}

#[test]
fn remote_gap_does_not_advance_observed_bucket_version() {
  let mut bucket = TopicRegistryBucket::new(owner("node-a", 1));
  let first_path = path_key("fraktor://sys/user/first");
  let second_path = path_key("fraktor://sys/user/second");
  let third_path = path_key("fraktor://sys/user/third");
  let first_target = subscriber("actor-first");
  let second_target = subscriber("actor-second");
  let third_target = subscriber("actor-third");
  let first_key = TopicRegistryEntryKey::Path { path: first_path.clone(), target: first_target.clone() };
  let second_key = TopicRegistryEntryKey::Path { path: second_path.clone(), target: second_target.clone() };
  let third_key = TopicRegistryEntryKey::Path { path: third_path.clone(), target: third_target.clone() };

  bucket.apply_remote_entry(
    third_key,
    TopicRegistryEntry::new(TopicRegistryVersion::new(3), TopicRegistryEntryKind::Path {
      path:   third_path,
      target: third_target,
    }),
  );
  assert_eq!(bucket.version(), TopicRegistryVersion::zero());

  bucket.apply_remote_entry(
    first_key,
    TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::Path {
      path:   first_path,
      target: first_target,
    }),
  );
  assert_eq!(bucket.version(), TopicRegistryVersion::new(1));

  bucket.apply_remote_entry(
    second_key,
    TopicRegistryEntry::new(TopicRegistryVersion::new(2), TopicRegistryEntryKind::Path {
      path:   second_path,
      target: second_target,
    }),
  );
  assert_eq!(bucket.version(), TopicRegistryVersion::new(3));
}

#[test]
fn remote_entry_with_observed_version_floor_advances_over_replaced_key_gap() {
  let mut bucket = TopicRegistryBucket::new(owner("node-a", 1));
  let path = path_key("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };

  bucket.apply_remote_entry_with_observed_version_floor(
    key,
    TopicRegistryEntry::new(TopicRegistryVersion::new(2), TopicRegistryEntryKind::Path { path, target }),
    TopicRegistryVersion::new(1),
  );

  assert_eq!(bucket.version(), TopicRegistryVersion::new(2));
}

#[test]
fn prune_removed_waits_until_known_peer_has_observed_tombstone_version() {
  let local_owner = owner("node-a", 1);
  let peer_owner = owner("node-b", 2);
  let mut bucket = TopicRegistryBucket::new(local_owner.clone());
  let path = path_key("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };
  bucket.put_path(path.clone(), target.clone());
  bucket.remove_path(path, target, 10);

  let stale_peer_status = TopicRegistryStatus::new(vec![(local_owner.clone(), TopicRegistryVersion::new(1))]);
  bucket.prune_removed(20, Duration::from_millis(10), &[stale_peer_status]);
  assert!(bucket.entry(&key).is_some());
  assert_eq!(bucket.version(), TopicRegistryVersion::new(2));

  let converged_peer_status = TopicRegistryStatus::new(vec![
    (local_owner, TopicRegistryVersion::new(2)),
    (peer_owner, TopicRegistryVersion::zero()),
  ]);
  bucket.prune_removed(20, Duration::from_millis(10), &[converged_peer_status]);
  assert!(bucket.entry(&key).is_none());
  assert_eq!(bucket.version(), TopicRegistryVersion::new(2));
}

#[test]
fn pruned_tombstone_watermark_is_compacted_to_version_floor() {
  let local_owner = owner("node-a", 1);
  let peer_owner = owner("node-b", 2);
  let mut bucket = TopicRegistryBucket::new(local_owner.clone());
  let path = path_key("fraktor://sys/user/service");
  let target = subscriber("actor-1");
  let key = TopicRegistryEntryKey::Path { path: path.clone(), target: target.clone() };
  bucket.put_path(path.clone(), target.clone());
  bucket.remove_path(path.clone(), target.clone(), 10);
  let converged_peer_status = TopicRegistryStatus::new(vec![
    (local_owner, TopicRegistryVersion::new(2)),
    (peer_owner, TopicRegistryVersion::zero()),
  ]);

  bucket.prune_removed(20, Duration::from_millis(10), &[converged_peer_status]);
  let stale_entry =
    TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::Path { path, target });

  assert!(bucket.entry(&key).is_none());
  assert_eq!(bucket.observed_version_floor, TopicRegistryVersion::new(2));
  assert!(!bucket.should_apply_remote_entry(&key, &stale_entry));
}

#[test]
fn delivery_view_excludes_removed_owner_bucket() {
  let local_owner = owner("node-a", 1);
  let mut bucket = TopicRegistryBucket::new(local_owner.clone());
  bucket.put_path(path_key("fraktor://sys/user/service"), subscriber("actor-1"));

  let active_view = bucket.delivery_view(&[local_owner]);
  let removed_view = bucket.delivery_view(&Vec::new());

  assert!(active_view.is_delivery_candidate());
  assert_eq!(active_view.entries().len(), 1);
  assert!(!removed_view.is_delivery_candidate());
  assert!(removed_view.entries().is_empty());
}
