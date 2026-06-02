use alloc::{string::String, vec, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::{sync::ArcShared, time::TimerInstant};

use super::*;
use crate::{
  BlockListProvider, ClusterProviderError,
  cluster_provider::{DiscoveredAuthority, DiscoveryResult},
};

struct EmptyBlockList;

impl BlockListProvider for EmptyBlockList {
  fn blocked_members(&self) -> Vec<String> {
    Vec::new()
  }
}

struct RecordingBlockList {
  blocked: Vec<String>,
}

impl RecordingBlockList {
  fn new(blocked: Vec<String>) -> Self {
    Self { blocked }
  }
}

impl BlockListProvider for RecordingBlockList {
  fn blocked_members(&self) -> Vec<String> {
    self.blocked.clone()
  }
}

fn observed_at(ticks: u64) -> TimerInstant {
  TimerInstant::from_ticks(ticks, Duration::from_secs(1))
}

fn authority(value: &str, source_identity: &str, ticks: u64) -> DiscoveredAuthority {
  DiscoveredAuthority::new(String::from(value), String::from(source_identity), observed_at(ticks))
}

fn empty_block_list() -> ArcShared<dyn BlockListProvider> {
  ArcShared::new(EmptyBlockList)
}

#[test]
fn discovered_authorities_publish_only_joined_delta_and_deduplicate_authority() {
  let mut mapper = DiscoveryTopologyMapper::new(empty_block_list());

  let update = mapper
    .apply(&DiscoveryResult::discovered(vec![
      authority("node-a.example:7331", "generic-discovery", 10),
      authority("node-a.example:7331", "aws-ecs", 10),
      authority("node-b.example:7331", "static-seed", 10),
    ]))
    .expect("first discovery should publish topology");

  assert_eq!(update.joined, vec![String::from("node-a.example:7331"), String::from("node-b.example:7331")]);
  assert!(update.left.is_empty());
  assert_eq!(update.members, update.joined);
  assert_eq!(update.topology.joined(), &update.joined);
  assert_eq!(update.topology.left(), &update.left);
  assert_eq!(update.observed_at, observed_at(10));
}

#[test]
fn refresh_publishes_only_joined_and_left_authority_delta() {
  let mut mapper = DiscoveryTopologyMapper::new(empty_block_list());
  let first_update = mapper.apply(&DiscoveryResult::discovered(vec![
    authority("node-a.example:7331", "generic-discovery", 10),
    authority("node-b.example:7331", "generic-discovery", 10),
  ]));
  assert!(first_update.is_some());

  let update = mapper
    .apply(&DiscoveryResult::discovered(vec![
      authority("node-b.example:7331", "generic-discovery", 11),
      authority("node-c.example:7331", "generic-discovery", 11),
    ]))
    .expect("changed discovery should publish topology");

  assert_eq!(update.joined, vec![String::from("node-c.example:7331")]);
  assert_eq!(update.left, vec![String::from("node-a.example:7331")]);
  assert_eq!(update.members, vec![String::from("node-b.example:7331"), String::from("node-c.example:7331")]);
  assert_eq!(update.observed_at, observed_at(11));
}

#[test]
fn repeated_success_without_delta_returns_no_topology_update() {
  let mut mapper = DiscoveryTopologyMapper::new(empty_block_list());
  let first_update =
    mapper.apply(&DiscoveryResult::discovered(vec![authority("node-a.example:7331", "generic-discovery", 10)]));
  assert!(first_update.is_some());

  let update = mapper.apply(&DiscoveryResult::discovered(vec![authority("node-a.example:7331", "aws-ecs", 11)]));

  assert!(update.is_none());
}

#[test]
fn failed_discovery_returns_no_update_and_preserves_previous_topology() {
  let mut mapper = DiscoveryTopologyMapper::new(empty_block_list());
  let first_update =
    mapper.apply(&DiscoveryResult::discovered(vec![authority("node-a.example:7331", "generic-discovery", 10)]));
  assert!(first_update.is_some());

  let failed = mapper.apply(&DiscoveryResult::failed(
    String::from("generic-discovery"),
    observed_at(11),
    ClusterProviderError::start_member("temporary backend failure"),
  ));
  assert!(failed.is_none());

  let update = mapper
    .apply(&DiscoveryResult::empty(String::from("generic-discovery"), observed_at(12)))
    .expect("empty success should remove previously discovered authorities");

  assert!(update.joined.is_empty());
  assert_eq!(update.left, vec![String::from("node-a.example:7331")]);
  assert!(update.members.is_empty());
  assert_eq!(update.observed_at, observed_at(12));
}

#[test]
fn block_list_provider_output_is_preserved_in_topology_update() {
  let block_list: ArcShared<dyn BlockListProvider> =
    ArcShared::new(RecordingBlockList::new(vec![String::from("blocked-a"), String::from("blocked-b")]));
  let mut mapper = DiscoveryTopologyMapper::new(block_list);

  let update = mapper
    .apply(&DiscoveryResult::discovered(vec![authority("node-a.example:7331", "aws-ecs", 10)]))
    .expect("discovery should publish topology");

  assert_eq!(update.blocked, vec![String::from("blocked-a"), String::from("blocked-b")]);
}

#[test]
fn static_generic_and_aws_sources_share_authority_only_topology_contract() {
  let static_update = DiscoveryTopologyMapper::new(empty_block_list())
    .apply(&DiscoveryResult::discovered(vec![authority("node-a.example:7331", "static-seed", 10)]))
    .expect("static seed should publish topology");
  let generic_update = DiscoveryTopologyMapper::new(empty_block_list())
    .apply(&DiscoveryResult::discovered(vec![authority("node-a.example:7331", "generic-discovery", 10)]))
    .expect("generic discovery should publish topology");
  let aws_update = DiscoveryTopologyMapper::new(empty_block_list())
    .apply(&DiscoveryResult::discovered(vec![authority("node-a.example:7331", "aws-ecs", 10)]))
    .expect("aws ecs discovery should publish topology");

  assert_eq!(static_update.joined, generic_update.joined);
  assert_eq!(generic_update.joined, aws_update.joined);
  assert_eq!(static_update.members, generic_update.members);
  assert_eq!(generic_update.members, aws_update.members);
}
