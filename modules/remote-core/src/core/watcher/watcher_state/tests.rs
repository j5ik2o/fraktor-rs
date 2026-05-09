use fraktor_actor_core_kernel_rs::actor::actor_path::{ActorPath, ActorPathParser};

use super::WatcherState;
use crate::core::{
  address::Address,
  failure_detector::PhiAccrualFailureDetector,
  watcher::{WatcherCommand, WatcherEffect},
};

fn test_factory(address: &Address) -> PhiAccrualFailureDetector {
  let acceptable_pause_ms = if address.host() == "10.0.0.1" { 1_000_000 } else { 0 };
  PhiAccrualFailureDetector::new(address.clone(), 5.0, 100, 10, acceptable_pause_ms, 100)
}

fn new_state() -> WatcherState {
  WatcherState::new(test_factory)
}

fn remote_target_at(host: &str, name: &str) -> ActorPath {
  let uri = alloc::format!("fraktor.tcp://remote-sys@{host}:2552/user/{name}");
  ActorPathParser::parse(&uri).expect("parse remote target")
}

fn local_watcher() -> ActorPath {
  ActorPath::root().child("user").child("me")
}

fn remote_node() -> Address {
  Address::new("remote-sys", "10.0.0.1", 2552)
}

fn terminated_count_for(effects: &[WatcherEffect], expected_target: &ActorPath) -> usize {
  effects
    .iter()
    .filter(|effect| matches!(effect, WatcherEffect::NotifyTerminated { target, .. } if target == expected_target))
    .count()
}

#[test]
fn watch_remote_target_uses_resource_aware_detector_factory() {
  // Given
  let mut state = new_state();
  let stable_target = remote_target_at("10.0.0.1", "stable");
  let strict_target = remote_target_at("10.0.0.2", "strict");
  let watcher = local_watcher();

  let stable_watch_effects =
    state.handle(WatcherCommand::Watch { target: stable_target.clone(), watcher: watcher.clone() });
  let strict_watch_effects = state.handle(WatcherCommand::Watch { target: strict_target.clone(), watcher });
  assert!(matches!(stable_watch_effects.as_slice(), [WatcherEffect::SendHeartbeat { .. }]));
  assert!(matches!(strict_watch_effects.as_slice(), [WatcherEffect::SendHeartbeat { .. }]));

  let stable_node = remote_node();
  let strict_node = Address::new("remote-sys", "10.0.0.2", 2552);
  for i in 0..10 {
    let stable_heartbeat_effects =
      state.handle(WatcherCommand::HeartbeatReceived { from: stable_node.clone(), now: i * 100 });
    let strict_heartbeat_effects =
      state.handle(WatcherCommand::HeartbeatReceived { from: strict_node.clone(), now: i * 100 });
    assert!(stable_heartbeat_effects.is_empty());
    assert!(strict_heartbeat_effects.is_empty());
  }

  // When
  let effects = state.handle(WatcherCommand::HeartbeatTick { now: 60_000 });

  // Then
  assert_eq!(terminated_count_for(&effects, &stable_target), 0);
  assert_eq!(terminated_count_for(&effects, &strict_target), 1);
}

#[test]
fn heartbeat_received_for_unknown_node_does_not_register_monitoring_state() {
  // Given
  let mut state = new_state();
  let node = remote_node();

  // When
  let effects = state.handle(WatcherCommand::HeartbeatReceived { from: node.clone(), now: 100 });
  let tick_effects = state.handle(WatcherCommand::HeartbeatTick { now: 1_000 });

  // Then
  assert!(effects.is_empty());
  assert!(tick_effects.is_empty());
}
