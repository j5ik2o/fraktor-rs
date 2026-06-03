use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::{
  activation::ClusterIdentity,
  pub_sub::{
    MediatorPathKey, PubSubSubscriber, PubSubTopic, TopicRegistryBucket, TopicRegistryEntryKey, TopicRegistryEntryKind,
    TopicRegistryVersion,
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

  let version = bucket.put_path(path.clone(), target.clone());

  let key = TopicRegistryEntryKey::Path { path, target };
  let entry = bucket.entry(&key).expect("entry");
  assert_eq!(version, TopicRegistryVersion::new(1));
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
  let removed_version = bucket.remove_path(path, target, 1_000);

  let entry = bucket.entry(&key).expect("tombstone");
  assert_eq!(removed_version, TopicRegistryVersion::new(2));
  assert!(matches!(entry.kind(), TopicRegistryEntryKind::Removed { removed_at_millis: 1_000 }));
  assert_eq!(bucket.prune_removed(1_500, Duration::from_millis(1_000)), 0);
  assert!(bucket.entry(&key).is_some());
  assert_eq!(bucket.version(), TopicRegistryVersion::new(2));
  assert_eq!(bucket.prune_removed(2_000, Duration::from_millis(1_000)), 1);
  assert!(bucket.entry(&key).is_none());
  assert_eq!(bucket.version(), TopicRegistryVersion::new(3));
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
  assert_eq!(bucket.prune_removed(20, Duration::from_millis(10)), 1);
  assert!(bucket.entry(&key).is_none());
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
