use alloc::{string::String, vec};
use core::{slice::from_ref, time::Duration};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::{
  activation::ClusterIdentity,
  pub_sub::{
    DistributedPubSubSettings, MediatorDeliveryIntent, MediatorDeliveryMode, MediatorPathKey, PubSubEnvelope,
    PubSubError, PubSubNoSubscriberBehavior, PubSubPathSemantics, PubSubRoutingMode, PubSubSubscriber, SendPathInput,
    SendToAllPathInput, TopicRegistryBucket,
  },
};

fn owner(name: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", name, 2552), uid)
}

fn subscriber(name: &str) -> PubSubSubscriber {
  PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", name).expect("identity"))
}

fn payload() -> PubSubEnvelope {
  PubSubEnvelope { serializer_id: 41, type_name: String::from("example.Message"), bytes: vec![1] }
}

fn path(value: &str) -> MediatorPathKey {
  MediatorPathKey::parse(value).expect("path")
}

fn settings(routing_mode: PubSubRoutingMode, behavior: PubSubNoSubscriberBehavior) -> DistributedPubSubSettings {
  DistributedPubSubSettings::try_new(None, routing_mode, Duration::from_secs(1), Duration::from_secs(30), 100, behavior)
    .expect("settings")
}

#[test]
fn send_selects_one_matching_path_entry() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let key = path("fraktor://sys/user/service");
  let first = subscriber("actor-1");
  let second = subscriber("actor-2");
  let mut local_bucket = TopicRegistryBucket::new(local.clone());
  let mut remote_bucket = TopicRegistryBucket::new(remote.clone());
  local_bucket.put_path(key.clone(), first.clone());
  remote_bucket.put_path(key.clone(), second.clone());
  let buckets = vec![local_bucket.delivery_view(from_ref(&local)), remote_bucket.delivery_view(&[remote])];
  let mut semantics =
    PubSubPathSemantics::new(settings(PubSubRoutingMode::Random, PubSubNoSubscriberBehavior::Drop), local);

  let intent = semantics.select_send_target(SendPathInput::new(key, payload(), false), &buckets).expect("intent");

  assert!(matches!(intent, MediatorDeliveryIntent::Deliver { mode: MediatorDeliveryMode::Send, .. }));
  assert_eq!(intent.targets().len(), 1);
}

#[test]
fn path_semantics_uses_canonical_relative_key() {
  let local = owner("node-a", 1);
  let registered = path("fraktor.tcp://sys@node-a:2552/user/service");
  let requested = path("fraktor.tcp://sys@node-b:2553/user/service");
  let target = subscriber("actor-1");
  let mut bucket = TopicRegistryBucket::new(local.clone());
  bucket.put_path(registered, target.clone());
  let buckets = vec![bucket.delivery_view(from_ref(&local))];
  let mut semantics =
    PubSubPathSemantics::new(settings(PubSubRoutingMode::Random, PubSubNoSubscriberBehavior::Drop), local);

  let intent = semantics.select_send_target(SendPathInput::new(requested, payload(), false), &buckets).expect("intent");

  assert_eq!(intent.targets(), &[target]);
}

#[test]
fn path_parse_failure_is_validation_failure() {
  let error = MediatorPathKey::parse("not-a-canonical-uri").expect_err("invalid path");

  assert!(matches!(error, PubSubError::InvalidPath { .. }));
}

#[test]
fn send_local_affinity_prefers_local_owner() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let key = path("fraktor://sys/user/service");
  let local_target = subscriber("local");
  let mut local_bucket = TopicRegistryBucket::new(local.clone());
  let mut remote_bucket = TopicRegistryBucket::new(remote.clone());
  local_bucket.put_path(key.clone(), local_target.clone());
  remote_bucket.put_path(key.clone(), subscriber("remote"));
  let buckets = vec![local_bucket.delivery_view(from_ref(&local)), remote_bucket.delivery_view(&[remote])];
  let mut semantics =
    PubSubPathSemantics::new(settings(PubSubRoutingMode::Random, PubSubNoSubscriberBehavior::Drop), local);

  let intent = semantics.select_send_target(SendPathInput::new(key, payload(), true), &buckets).expect("intent");

  assert_eq!(intent.targets(), &[local_target]);
}

#[test]
fn send_round_robin_uses_settings_routing_mode() {
  let local = owner("node-a", 1);
  let key = path("fraktor://sys/user/service");
  let first = subscriber("actor-1");
  let second = subscriber("actor-2");
  let mut bucket = TopicRegistryBucket::new(local.clone());
  bucket.put_path(key.clone(), first.clone());
  bucket.put_path(key.clone(), second.clone());
  let buckets = vec![bucket.delivery_view(from_ref(&local))];
  let mut semantics =
    PubSubPathSemantics::new(settings(PubSubRoutingMode::RoundRobin, PubSubNoSubscriberBehavior::Drop), local);

  let first_intent =
    semantics.select_send_target(SendPathInput::new(key.clone(), payload(), false), &buckets).expect("first");
  let second_intent =
    semantics.select_send_target(SendPathInput::new(key, payload(), false), &buckets).expect("second");

  assert_ne!(first_intent.targets(), second_intent.targets());
}

#[test]
fn send_to_all_selects_all_matching_path_entries() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let key = path("fraktor://sys/user/service");
  let mut local_bucket = TopicRegistryBucket::new(local.clone());
  let mut remote_bucket = TopicRegistryBucket::new(remote.clone());
  local_bucket.put_path(key.clone(), subscriber("local"));
  remote_bucket.put_path(key.clone(), subscriber("remote"));
  let buckets = vec![local_bucket.delivery_view(from_ref(&local)), remote_bucket.delivery_view(&[remote])];
  let semantics =
    PubSubPathSemantics::new(settings(PubSubRoutingMode::Random, PubSubNoSubscriberBehavior::Drop), local);

  let intent =
    semantics.select_send_to_all_targets(SendToAllPathInput::new(key, payload(), false), &buckets).expect("intent");

  assert!(matches!(intent, MediatorDeliveryIntent::Deliver { mode: MediatorDeliveryMode::SendToAll, .. }));
  assert_eq!(intent.targets().len(), 2);
}

#[test]
fn send_to_all_all_but_self_excludes_local_owner() {
  let local = owner("node-a", 1);
  let remote = owner("node-b", 2);
  let key = path("fraktor://sys/user/service");
  let remote_target = subscriber("remote");
  let mut local_bucket = TopicRegistryBucket::new(local.clone());
  let mut remote_bucket = TopicRegistryBucket::new(remote.clone());
  local_bucket.put_path(key.clone(), subscriber("local"));
  remote_bucket.put_path(key.clone(), remote_target.clone());
  let buckets = vec![local_bucket.delivery_view(from_ref(&local)), remote_bucket.delivery_view(&[remote])];
  let semantics =
    PubSubPathSemantics::new(settings(PubSubRoutingMode::Random, PubSubNoSubscriberBehavior::Drop), local);

  let intent =
    semantics.select_send_to_all_targets(SendToAllPathInput::new(key, payload(), true), &buckets).expect("intent");

  assert_eq!(intent.targets(), &[remote_target]);
}

#[test]
fn no_subscriber_uses_drop_or_dead_letter_intent() {
  let local = owner("node-a", 1);
  let key = path("fraktor://sys/user/missing");
  let mut drop_semantics =
    PubSubPathSemantics::new(settings(PubSubRoutingMode::Random, PubSubNoSubscriberBehavior::Drop), local.clone());
  let mut dead_letter_semantics =
    PubSubPathSemantics::new(settings(PubSubRoutingMode::Random, PubSubNoSubscriberBehavior::DeadLetter), local);

  let dropped =
    drop_semantics.select_send_target(SendPathInput::new(key.clone(), payload(), false), &[]).expect("dropped");
  let dead_letter =
    dead_letter_semantics.select_send_target(SendPathInput::new(key, payload(), false), &[]).expect("dead letter");

  assert!(matches!(dropped, MediatorDeliveryIntent::Dropped { .. }));
  assert!(matches!(dead_letter, MediatorDeliveryIntent::DeadLetter { .. }));
}

#[test]
fn round_robin_no_subscriber_uses_configured_no_subscriber_intent() {
  let local = owner("node-a", 1);
  let key = path("fraktor://sys/user/missing");
  let mut semantics =
    PubSubPathSemantics::new(settings(PubSubRoutingMode::RoundRobin, PubSubNoSubscriberBehavior::DeadLetter), local);

  let intent = semantics.select_send_target(SendPathInput::new(key, payload(), false), &[]).expect("intent");

  assert!(matches!(intent, MediatorDeliveryIntent::DeadLetter { .. }));
}
