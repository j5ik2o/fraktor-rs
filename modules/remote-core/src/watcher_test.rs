use alloc::vec::Vec;

use fraktor_actor_core_kernel_rs::actor::actor_path::{ActorPath, ActorPathParser};

use crate::{
  address::Address,
  failure_detector::PhiAccrualFailureDetector,
  watcher::{WatcherCommand, WatcherEffect, WatcherState},
};

fn test_factory(address: &Address) -> PhiAccrualFailureDetector {
  PhiAccrualFailureDetector::new(address.clone(), 5.0, 100, 10, 0, 100)
}

fn new_state() -> WatcherState {
  WatcherState::new(test_factory)
}

fn remote_target(n: u16) -> ActorPath {
  let uri = alloc::format!("fraktor.tcp://remote-sys@10.0.0.{n}:2552/user/worker");
  ActorPathParser::parse(&uri).expect("parse remote target")
}

fn remote_target_custom(system: &str, host: &str, port: u16, seg: &str) -> ActorPath {
  let uri = alloc::format!("fraktor.tcp://{system}@{host}:{port}/user/{seg}");
  ActorPathParser::parse(&uri).expect("parse")
}

fn local_watcher() -> ActorPath {
  ActorPath::root().child("user").child("me")
}

fn address_of(system: &str, host: &str, port: u16) -> Address {
  Address::new(system, host, port)
}

fn address_terminated_count_for(effects: &[WatcherEffect], expected_node: &Address) -> usize {
  effects
    .iter()
    .filter(|effect| matches!(effect, WatcherEffect::AddressTerminated { node, .. } if node == expected_node))
    .count()
}

// ---------------------------------------------------------------------------
// Watch / Unwatch bookkeeping
// ---------------------------------------------------------------------------

#[test]
fn watch_remote_target_registers_pair_and_sends_initial_heartbeat() {
  let mut state = new_state();
  let target = remote_target(1);
  let watcher = local_watcher();
  let effects = state.handle(WatcherCommand::Watch { target: target.clone(), watcher: watcher.clone() });

  assert_eq!(state.watch_pair_count(), 1);
  assert_eq!(state.node_count(), 1);
  assert!(effects.iter().any(|effect| matches!(
    effect,
    WatcherEffect::SendWatch {
      target: effect_target,
      watcher: effect_watcher,
    } if effect_target == &target && effect_watcher == &watcher
  )));
  assert!(effects.iter().any(|effect| matches!(effect, WatcherEffect::SendHeartbeat { .. })));
}

#[test]
fn watching_same_pair_is_idempotent() {
  let mut state = new_state();
  let target = remote_target(1);
  let watcher = local_watcher();
  let _ = state.handle(WatcherCommand::Watch { target: target.clone(), watcher: watcher.clone() });
  let effects = state.handle(WatcherCommand::Watch { target, watcher });
  assert_eq!(state.watch_pair_count(), 1);
  assert!(!effects.iter().any(|effect| matches!(effect, WatcherEffect::SendWatch { .. })));
}

#[test]
fn watching_local_path_is_ignored() {
  let mut state = new_state();
  // `local_watcher()` is an ActorPath without authority → local path.
  let local_target = ActorPath::root().child("user").child("localt");
  let effects = state.handle(WatcherCommand::Watch { target: local_target, watcher: local_watcher() });
  assert!(effects.is_empty());
  assert_eq!(state.watch_pair_count(), 0);
  assert_eq!(state.node_count(), 0);
}

#[test]
fn unwatch_removes_the_pair_and_cleans_up_node_when_empty() {
  let mut state = new_state();
  let target = remote_target(1);
  let watcher = local_watcher();
  let _ = state.handle(WatcherCommand::Watch { target: target.clone(), watcher: watcher.clone() });
  assert_eq!(state.node_count(), 1);

  let effects = state.handle(WatcherCommand::Unwatch { target: target.clone(), watcher: watcher.clone() });
  assert_eq!(state.watch_pair_count(), 0);
  assert_eq!(state.node_count(), 0);
  assert!(effects.iter().any(|effect| matches!(
    effect,
    WatcherEffect::SendUnwatch {
      target: effect_target,
      watcher: effect_watcher,
    } if effect_target == &target && effect_watcher == &watcher
  )));
}

#[test]
fn unwatch_unknown_pair_emits_no_effect() {
  let mut state = new_state();
  let effects = state.handle(WatcherCommand::Unwatch { target: remote_target(1), watcher: local_watcher() });

  assert!(effects.is_empty());
}

#[test]
fn multiple_targets_on_same_node_share_detector() {
  let mut state = new_state();
  let t1 = remote_target_custom("sys", "10.0.0.1", 2552, "a");
  let t2 = remote_target_custom("sys", "10.0.0.1", 2552, "b");
  let w = local_watcher();
  let _ = state.handle(WatcherCommand::Watch { target: t1, watcher: w.clone() });
  let _ = state.handle(WatcherCommand::Watch { target: t2, watcher: w });
  assert_eq!(state.node_count(), 1, "both targets live on the same node");
  assert_eq!(state.watch_pair_count(), 2);
}

// ---------------------------------------------------------------------------
// Heartbeat propagation to the failure detector
// ---------------------------------------------------------------------------

#[test]
fn heartbeat_received_updates_detector_for_known_node() {
  let mut state = new_state();
  let target = remote_target(1);
  let watcher = local_watcher();
  let _ = state.handle(WatcherCommand::Watch { target, watcher });

  let node = address_of("remote-sys", "10.0.0.1", 2552);
  // Send a few heartbeats and then tick — the node must remain available.
  for i in 0..5 {
    let _ = state.handle(WatcherCommand::HeartbeatReceived { from: node.clone(), now: i * 100 });
  }
  let effects = state.handle(WatcherCommand::HeartbeatTick { now: 500 });
  // Effects should include periodic SendHeartbeat but **no** NotifyTerminated.
  assert!(effects.iter().all(|e| !matches!(e, WatcherEffect::NotifyTerminated { .. })));
}

#[test]
fn heartbeat_response_received_records_initial_uid_and_rewatches_targets() {
  let mut state = new_state();
  let target = remote_target(1);
  let watcher = local_watcher();
  let _ = state.handle(WatcherCommand::Watch { target: target.clone(), watcher });

  let node = address_of("remote-sys", "10.0.0.1", 2552);
  let effects = state.handle(WatcherCommand::HeartbeatResponseReceived { from: node.clone(), uid: 42, now: 100 });

  assert!(effects.iter().any(|effect| matches!(
    effect,
    WatcherEffect::RewatchRemoteTargets {
      node: effect_node,
      watches
    } if effect_node == &node && watches == &alloc::vec![(target.clone(), local_watcher())]
  )));
}

#[test]
fn heartbeat_response_rewatches_all_watchers_for_target() {
  let mut state = new_state();
  let target = remote_target(1);
  let watcher_a = ActorPath::root().child("user").child("watcher-a");
  let watcher_b = ActorPath::root().child("user").child("watcher-b");
  let _effects = state.handle(WatcherCommand::Watch { target: target.clone(), watcher: watcher_a.clone() });
  let _effects = state.handle(WatcherCommand::Watch { target: target.clone(), watcher: watcher_b.clone() });
  let node = address_of("remote-sys", "10.0.0.1", 2552);

  let effects = state.handle(WatcherCommand::HeartbeatResponseReceived { from: node, uid: 42, now: 100 });

  assert!(effects.iter().any(|effect| matches!(
    effect,
    WatcherEffect::RewatchRemoteTargets {
      watches,
      ..
    } if watches.contains(&(target.clone(), watcher_a.clone())) && watches.contains(&(target.clone(), watcher_b.clone()))
  )));
}

#[test]
fn heartbeat_response_received_with_same_uid_does_not_rewatch() {
  let mut state = new_state();
  let target = remote_target(1);
  let watcher = local_watcher();
  let _ = state.handle(WatcherCommand::Watch { target, watcher });
  let node = address_of("remote-sys", "10.0.0.1", 2552);

  let _ = state.handle(WatcherCommand::HeartbeatResponseReceived { from: node.clone(), uid: 42, now: 100 });
  let effects = state.handle(WatcherCommand::HeartbeatResponseReceived { from: node, uid: 42, now: 200 });

  assert!(effects.iter().all(|effect| !matches!(effect, WatcherEffect::RewatchRemoteTargets { .. })));
}

#[test]
fn heartbeat_response_received_with_changed_uid_rewatches_targets() {
  let mut state = new_state();
  let target = remote_target(1);
  let watcher = local_watcher();
  let _ = state.handle(WatcherCommand::Watch { target: target.clone(), watcher });
  let node = address_of("remote-sys", "10.0.0.1", 2552);

  let _ = state.handle(WatcherCommand::HeartbeatResponseReceived { from: node.clone(), uid: 42, now: 100 });
  let effects = state.handle(WatcherCommand::HeartbeatResponseReceived { from: node.clone(), uid: 43, now: 200 });

  assert!(effects.iter().any(|effect| matches!(
    effect,
    WatcherEffect::RewatchRemoteTargets {
      node: effect_node,
      watches
    } if effect_node == &node && watches == &alloc::vec![(target.clone(), local_watcher())]
  )));
}

#[test]
fn heartbeat_response_received_for_unknown_node_is_ignored() {
  let mut state = new_state();
  let unknown = address_of("remote-sys", "10.0.0.9", 2552);

  let effects = state.handle(WatcherCommand::HeartbeatResponseReceived { from: unknown, uid: 42, now: 100 });

  assert!(effects.is_empty());
}

#[test]
fn heartbeat_response_received_after_notification_reopens_the_detector() {
  let mut state = new_state();
  let _ = state.handle(WatcherCommand::Watch { target: remote_target(1), watcher: local_watcher() });
  let node = address_of("remote-sys", "10.0.0.1", 2552);
  for i in 0..10 {
    let _ = state.handle(WatcherCommand::HeartbeatResponseReceived { from: node.clone(), uid: 42, now: i * 100 });
  }
  let _first = state.handle(WatcherCommand::HeartbeatTick { now: 60_000 });

  let _ = state.handle(WatcherCommand::HeartbeatResponseReceived { from: node, uid: 42, now: 70_000 });
  let effects = state.handle(WatcherCommand::HeartbeatTick { now: 200_000 });
  let terminated_again = effects.iter().filter(|e| matches!(e, WatcherEffect::NotifyTerminated { .. })).count();
  assert_eq!(terminated_again, 1);
}

#[test]
fn heartbeat_tick_emits_send_heartbeat_for_every_tracked_node() {
  let mut state = new_state();
  let _ = state.handle(WatcherCommand::Watch {
    target:  remote_target_custom("sys", "10.0.0.1", 2552, "a"),
    watcher: local_watcher(),
  });
  let _ = state.handle(WatcherCommand::Watch {
    target:  remote_target_custom("sys", "10.0.0.2", 2552, "b"),
    watcher: local_watcher(),
  });

  let effects = state.handle(WatcherCommand::HeartbeatTick { now: 0 });
  let heartbeat_count = effects.iter().filter(|e| matches!(e, WatcherEffect::SendHeartbeat { .. })).count();
  assert_eq!(heartbeat_count, 2);
}

// ---------------------------------------------------------------------------
// Failure detection → termination / quarantine notification
// ---------------------------------------------------------------------------

#[test]
fn long_silence_triggers_terminated_and_quarantined_effects() {
  // Pekko reference: RemoteWatcher.scala:197-207 publishes AddressTerminated after failure detection;
  // RemoteDaemon.scala:82 and 232 subscribe and react to that signal.
  let mut state = new_state();
  let target = remote_target(1);
  let watcher = local_watcher();
  let _ = state.handle(WatcherCommand::Watch { target: target.clone(), watcher: watcher.clone() });

  let node = address_of("remote-sys", "10.0.0.1", 2552);
  // Establish a stable heartbeat cadence so the detector has real data.
  for i in 0..10 {
    let _ = state.handle(WatcherCommand::HeartbeatReceived { from: node.clone(), now: i * 100 });
  }
  let _ = state.handle(WatcherCommand::HeartbeatTick { now: 1_000 });

  // Long silence after the last heartbeat.
  let effects = state.handle(WatcherCommand::HeartbeatTick { now: 60_000 });
  let terminated: Vec<_> = effects
    .iter()
    .filter_map(|e| match e {
      | WatcherEffect::NotifyTerminated { target, watchers } => Some((target.clone(), watchers.clone())),
      | _ => None,
    })
    .collect();
  let address_terminated: Vec<_> = effects
    .iter()
    .filter_map(|e| match e {
      | WatcherEffect::AddressTerminated { node, reason, observed_at_millis } => {
        Some((node.clone(), reason.clone(), *observed_at_millis))
      },
      | _ => None,
    })
    .collect();
  let quarantined: Vec<_> = effects.iter().filter(|e| matches!(e, WatcherEffect::NotifyQuarantined { .. })).collect();
  assert_eq!(terminated.len(), 1);
  assert_eq!(terminated[0].0, target);
  assert_eq!(terminated[0].1, [watcher]);
  assert_eq!(address_terminated.len(), 1);
  assert_eq!(address_terminated[0].0, node);
  assert_eq!(address_terminated[0].1, "Deemed unreachable by remote failure detector");
  assert_eq!(address_terminated[0].2, 60_000);
  assert_eq!(quarantined.len(), 1);
}

#[test]
fn address_terminated_and_notify_terminated_are_not_duplicated_across_ticks() {
  let mut state = new_state();
  let _ = state.handle(WatcherCommand::Watch { target: remote_target(1), watcher: local_watcher() });
  let node = address_of("remote-sys", "10.0.0.1", 2552);
  for i in 0..10 {
    let _ = state.handle(WatcherCommand::HeartbeatReceived { from: node.clone(), now: i * 100 });
  }

  let first = state.handle(WatcherCommand::HeartbeatTick { now: 60_000 });
  let second = state.handle(WatcherCommand::HeartbeatTick { now: 70_000 });

  let first_terminated = first.iter().filter(|e| matches!(e, WatcherEffect::NotifyTerminated { .. })).count();
  let second_terminated = second.iter().filter(|e| matches!(e, WatcherEffect::NotifyTerminated { .. })).count();
  let first_address_terminated = address_terminated_count_for(&first, &node);
  let second_address_terminated = address_terminated_count_for(&second, &node);
  assert_eq!(first_terminated, 1);
  assert_eq!(second_terminated, 0, "second tick must not re-emit the same termination");
  assert_eq!(first_address_terminated, 1);
  assert_eq!(second_address_terminated, 0, "second tick must not re-emit the same address termination");
}

#[test]
fn heartbeat_received_after_notification_reopens_the_detector() {
  let mut state = new_state();
  let _ = state.handle(WatcherCommand::Watch { target: remote_target(1), watcher: local_watcher() });
  let node = address_of("remote-sys", "10.0.0.1", 2552);
  for i in 0..10 {
    let _ = state.handle(WatcherCommand::HeartbeatReceived { from: node.clone(), now: i * 100 });
  }
  let _first = state.handle(WatcherCommand::HeartbeatTick { now: 60_000 });

  // A fresh heartbeat re-opens the detector and clears the notified flag.
  let _ = state.handle(WatcherCommand::HeartbeatReceived { from: node.clone(), now: 70_000 });
  // Another prolonged silence should yield a fresh termination notification.
  let effects = state.handle(WatcherCommand::HeartbeatTick { now: 200_000 });
  let terminated_again = effects.iter().filter(|e| matches!(e, WatcherEffect::NotifyTerminated { .. })).count();
  let address_terminated_again = address_terminated_count_for(&effects, &node);
  assert_eq!(terminated_again, 1);
  assert_eq!(address_terminated_again, 1);
}
