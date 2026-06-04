use alloc::{string::String, vec, vec::Vec};
use core::time::Duration;

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::{
  membership::{CurrentClusterState, DataCenter, MembershipVersion, NodeRecord, NodeStatus},
  pub_sub::{DistributedPubSubSettings, PubSubError, PubSubNoSubscriberBehavior, PubSubRoutingMode},
};

#[test]
fn default_settings_match_mediator_contract() {
  let settings = DistributedPubSubSettings::default();

  assert_eq!(settings.role(), None);
  assert_eq!(settings.routing_mode(), PubSubRoutingMode::Random);
  assert_eq!(settings.gossip_interval(), Duration::from_secs(1));
  assert_eq!(settings.removed_entry_ttl(), Duration::from_secs(120));
  assert_eq!(settings.max_delta_elements(), 3000);
  assert_eq!(settings.no_subscriber_behavior(), PubSubNoSubscriberBehavior::Drop);
}

#[test]
fn try_new_rejects_zero_max_delta_elements() {
  let result = DistributedPubSubSettings::try_new(
    None,
    PubSubRoutingMode::Random,
    Duration::from_secs(1),
    Duration::from_secs(120),
    0,
    PubSubNoSubscriberBehavior::Drop,
  );

  assert!(
    matches!(result, Err(PubSubError::InvalidSettings { reason }) if reason == "max_delta_elements must be greater than zero")
  );
}

#[test]
fn routing_mode_accepts_supported_names() {
  assert_eq!(PubSubRoutingMode::try_from_name("random"), Ok(PubSubRoutingMode::Random));
  assert_eq!(PubSubRoutingMode::try_from_name("round-robin"), Ok(PubSubRoutingMode::RoundRobin));
}

#[test]
fn routing_mode_rejects_unsupported_name() {
  let result = PubSubRoutingMode::try_from_name("consistent-hashing");

  assert!(
    matches!(result, Err(PubSubError::InvalidSettings { reason }) if reason == "unsupported routing mode: consistent-hashing")
  );
}

#[test]
fn try_new_keeps_dead_letter_behavior() {
  let settings = DistributedPubSubSettings::try_new(
    None,
    PubSubRoutingMode::Random,
    Duration::from_secs(1),
    Duration::from_secs(120),
    16,
    PubSubNoSubscriberBehavior::DeadLetter,
  )
  .expect("settings should be valid");

  assert_eq!(settings.no_subscriber_behavior(), PubSubNoSubscriberBehavior::DeadLetter);
}

#[test]
fn role_filter_keeps_only_active_members_with_matching_role() {
  let settings = DistributedPubSubSettings::try_new(
    Some(String::from("pubsub")),
    PubSubRoutingMode::RoundRobin,
    Duration::from_millis(500),
    Duration::from_secs(30),
    16,
    PubSubNoSubscriberBehavior::DeadLetter,
  )
  .expect("settings should be valid");
  let state = CurrentClusterState::new(
    vec![
      record("node-a", 1, NodeStatus::Up, vec![String::from("pubsub")]),
      record("node-b", 2, NodeStatus::Up, vec![String::from("worker")]),
      record("node-c", 3, NodeStatus::Removed, vec![String::from("pubsub")]),
    ],
    Vec::new(),
    Vec::new(),
    None,
    Default::default(),
  );

  let candidates = settings.mediator_candidates(&state);

  assert_eq!(candidates.iter().map(|record| record.node_id.as_str()).collect::<Vec<_>>(), vec!["node-a"]);
}

fn record(node_id: &str, uid: u64, status: NodeStatus, roles: Vec<String>) -> NodeRecord {
  NodeRecord::new_with_identity(
    UniqueAddress::new(Address::new("cluster", node_id, 2552), uid),
    DataCenter::default(),
    String::from(node_id),
    status,
    MembershipVersion::new(uid),
    String::from("1.0.0"),
    roles,
  )
}
