use alloc::vec;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::{
  activation::ClusterIdentity,
  membership::GossipPayloadKind,
  pub_sub::{
    MediatorPathKey, PubSubGossipHandoff, PubSubSubscriber, TopicRegistryDelta, TopicRegistryDeltaEntry,
    TopicRegistryEntry, TopicRegistryEntryKey, TopicRegistryEntryKind, TopicRegistryGossipPayload, TopicRegistryStatus,
    TopicRegistryVersion,
  },
};

fn owner(name: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", name, 2552), uid)
}

#[test]
fn status_handoff_uses_logical_pubsub_status_kind_without_envelope() {
  let status = TopicRegistryStatus::new(vec![(owner("node-a", 1), TopicRegistryVersion::new(7))]);

  let handoff = PubSubGossipHandoff::status(status.clone());

  assert_eq!(handoff.payload_kind(), GossipPayloadKind::PubSubRegistryStatus);
  assert_eq!(handoff.payload(), &TopicRegistryGossipPayload::Status(status));
}

#[test]
fn delta_handoff_uses_logical_pubsub_delta_kind_without_transport_tag() {
  let owner = owner("node-a", 1);
  let key = TopicRegistryEntryKey::Path {
    path:   MediatorPathKey::parse("fraktor://sys/user/service").expect("path"),
    target: PubSubSubscriber::ClusterIdentity(ClusterIdentity::new("kind", "actor-1").expect("identity")),
  };
  let delta = TopicRegistryDelta::new(vec![TopicRegistryDeltaEntry::new(
    owner,
    key,
    TopicRegistryEntry::new(TopicRegistryVersion::new(1), TopicRegistryEntryKind::Removed { removed_at_millis: 10 }),
  )]);

  let handoff = PubSubGossipHandoff::delta(delta.clone());

  assert_eq!(handoff.payload_kind(), GossipPayloadKind::PubSubRegistryDelta);
  assert_eq!(handoff.payload(), &TopicRegistryGossipPayload::Delta(delta));
}
