use alloc::{collections::BTreeMap, string::String, vec};
use core::{slice::from_ref, time::Duration};

use fraktor_remote_core_rs::address::{Address, UniqueAddress};

use crate::{
  membership::{CurrentClusterState, DataCenter, MembershipVersion, NodeRecord, NodeStatus},
  pub_sub::{DistributedPubSubConfig, MediatorPeers, PubSubNoSubscriberBehavior, PubSubRoutingMode},
};

fn unique_address(host: &str, uid: u64) -> UniqueAddress {
  UniqueAddress::new(Address::new("cluster", host, 2552), uid)
}

fn record(host: &str, uid: u64, status: NodeStatus, roles: Vec<String>) -> NodeRecord {
  NodeRecord::new_with_identity(
    unique_address(host, uid),
    DataCenter::default(),
    String::from(host),
    status,
    MembershipVersion::new(uid),
    String::from("1.0.0"),
    roles,
  )
}

#[test]
fn peers_use_role_filter_and_active_member_status() {
  let config = DistributedPubSubConfig::try_new(
    Some(String::from("pubsub")),
    PubSubRoutingMode::Random,
    Duration::from_secs(1),
    Duration::from_secs(30),
    100,
    PubSubNoSubscriberBehavior::Drop,
  )
  .expect("config");
  let active = record("node-a", 1, NodeStatus::Up, vec![String::from("pubsub")]);
  let wrong_role = record("node-b", 2, NodeStatus::Up, vec![String::from("backend")]);
  let removed = record("node-c", 3, NodeStatus::Removed, vec![String::from("pubsub")]);
  let leaving = record("node-d", 4, NodeStatus::Leaving, vec![String::from("pubsub")]);
  let state =
    CurrentClusterState::new(vec![active.clone(), wrong_role, removed, leaving], vec![], vec![], None, BTreeMap::new());

  let peers = MediatorPeers::from_state(&config, &state);

  assert_eq!(peers.active_owners(), from_ref(&active.unique_address));
  assert!(peers.contains(&active.unique_address));
}
