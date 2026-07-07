use alloc::{string::String, vec};
use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

use super::{
  ClusterSingletonManager, ClusterSingletonManagerEffect, ClusterSingletonManagerMessage, ClusterSingletonManagerPhase,
};
use crate::{
  membership::{MembershipVersion, NodeRecord, NodeStatus},
  singleton::{ClusterSingletonManagerConfig, SingletonStuckPhase},
};

fn now(ticks: u64) -> TimerInstant {
  TimerInstant::from_ticks(ticks, Duration::from_secs(1))
}

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

#[test]
fn oldest_member_starts_singleton_on_first_topology() {
  let mut manager = ClusterSingletonManager::new(ClusterSingletonManagerConfig::new(), "n1:4000");
  let members = vec![make_record("n1:4000", 1), make_record("n2:4000", 2)];
  let outcome = manager.apply_topology(&members, now(0));

  assert_eq!(manager.phase(), ClusterSingletonManagerPhase::Oldest);
  assert!(manager.singleton_running());
  assert_eq!(outcome.effects, vec![ClusterSingletonManagerEffect::StartSingleton]);
}

#[test]
fn younger_member_waits_without_starting_singleton() {
  let mut manager = ClusterSingletonManager::new(ClusterSingletonManagerConfig::new(), "n2:4000");
  let members = vec![make_record("n1:4000", 1), make_record("n2:4000", 2)];
  let outcome = manager.apply_topology(&members, now(0));

  assert_eq!(manager.phase(), ClusterSingletonManagerPhase::Younger);
  assert!(!manager.singleton_running());
  assert!(outcome.effects.is_empty());
}

#[test]
fn hand_over_to_me_moves_oldest_into_handing_over() {
  let mut manager = ClusterSingletonManager::new(ClusterSingletonManagerConfig::new(), "n1:4000");
  let members = vec![make_record("n1:4000", 1)];
  let _ = manager.apply_topology(&members, now(0));

  let outcome = manager.handle_message(ClusterSingletonManagerMessage::HandOverToMe);
  assert_eq!(manager.phase(), ClusterSingletonManagerPhase::HandingOver);
  assert!(!manager.singleton_running());
  assert_eq!(outcome.effects, vec![ClusterSingletonManagerEffect::StopSingleton]);
}

#[test]
fn hand_over_done_starts_singleton_from_becoming_oldest() {
  let mut manager = ClusterSingletonManager::new(ClusterSingletonManagerConfig::new(), "n2:4000");
  manager.phase = ClusterSingletonManagerPhase::BecomingOldest;

  let outcome = manager.handle_message(ClusterSingletonManagerMessage::HandOverDone);
  assert_eq!(manager.phase(), ClusterSingletonManagerPhase::Oldest);
  assert!(manager.singleton_running());
  assert_eq!(outcome.effects, vec![ClusterSingletonManagerEffect::StartSingleton]);
}

#[test]
fn poll_publishes_stuck_event_after_retry_budget() {
  let config = ClusterSingletonManagerConfig::new().with_min_hand_over_retries(0);
  let mut manager = ClusterSingletonManager::new(config, "n2:4000");
  manager.phase = ClusterSingletonManagerPhase::BecomingOldest;
  manager.previous_oldest = Some(String::from("n1:4000"));
  manager.next_retry_at = Some(now(0));
  manager.hand_over_retry_count = manager.config.max_hand_over_retries();

  let outcome = manager.poll(now(2));
  assert_eq!(outcome.effects, vec![ClusterSingletonManagerEffect::PublishHandOverStuck {
    phase: SingletonStuckPhase::BecomingOldest,
  }]);
}
