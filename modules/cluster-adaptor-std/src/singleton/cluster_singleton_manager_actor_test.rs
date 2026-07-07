use alloc::{string::String, vec};
use core::time::Duration;

use fraktor_cluster_core_kernel_rs::{
  membership::{MembershipVersion, NodeRecord, NodeStatus},
  singleton::{ClusterSingletonManagerConfig, ClusterSingletonManagerEffect, ClusterSingletonManagerPhase},
};
use fraktor_utils_core_rs::time::TimerInstant;

use super::ClusterSingletonManagerActor;

fn make_record(authority: &str, join_v: u64) -> NodeRecord {
  NodeRecord::new(
    String::from("node"),
    String::from(authority),
    NodeStatus::Up,
    MembershipVersion::new(join_v),
    String::from("1.0.0"),
    vec![],
  )
}

fn now(ticks: u64) -> TimerInstant {
  TimerInstant::from_ticks(ticks, Duration::from_secs(1))
}

#[test]
fn manager_actor_starts_singleton_for_oldest_member() {
  let mut actor = ClusterSingletonManagerActor::new(ClusterSingletonManagerConfig::new(), "n1:4000");

  let outcome = actor.apply_topology(&[make_record("n1:4000", 1)], now(0));
  assert_eq!(actor.manager().phase(), ClusterSingletonManagerPhase::Oldest);
  assert_eq!(outcome.effects, vec![ClusterSingletonManagerEffect::StartSingleton]);
}
