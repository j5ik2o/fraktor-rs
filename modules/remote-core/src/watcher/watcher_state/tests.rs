use fraktor_actor_core_rs::core::kernel::actor::actor_path::{ActorPath, ActorPathParser};

use super::WatcherState;
use crate::{
  address::Address,
  failure_detector::PhiAccrualFailureDetector,
  watcher::{WatcherCommand, WatcherEffect},
};

fn test_factory() -> PhiAccrualFailureDetector {
  PhiAccrualFailureDetector::new(5.0, 100, 10, 0, 100)
}

fn new_state() -> WatcherState {
  WatcherState::new(test_factory)
}

fn remote_target() -> ActorPath {
  ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse remote target")
}

fn local_watcher() -> ActorPath {
  ActorPath::root().child("user").child("me")
}

fn remote_node() -> Address {
  Address::new("remote-sys", "10.0.0.1", 2552)
}

fn detector_address_for_node<'a>(state: &'a WatcherState, node: &Address) -> Option<&'a str> {
  state.detectors.get(node).and_then(|detector| detector.monitored_address.as_deref())
}

#[test]
fn watch_remote_target_configures_detector_address() {
  let mut state = new_state();
  let target = remote_target();
  let watcher = local_watcher();

  let effects = state.handle(WatcherCommand::Watch { target, watcher });

  let node = remote_node();
  assert!(matches!(effects.as_slice(), [WatcherEffect::SendHeartbeat { .. }]));
  assert_eq!(detector_address_for_node(&state, &node), Some("remote-sys@10.0.0.1:2552"));
}

#[test]
fn heartbeat_received_for_unknown_node_configures_detector_address() {
  let mut state = new_state();
  let node = remote_node();

  let effects = state.handle(WatcherCommand::HeartbeatReceived { from: node.clone(), now: 100 });

  assert!(effects.is_empty());
  assert_eq!(detector_address_for_node(&state, &node), Some("remote-sys@10.0.0.1:2552"));
}
