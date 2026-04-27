use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::actor::actor_path::{ActorPath, ActorPathParser};
use fraktor_remote_core_rs::core::{
  address::Address,
  watcher::{WatcherCommand, WatcherEffect},
};
use tokio::{sync::mpsc, time::Instant};

use crate::std::watcher_actor::{base::WatcherActor, heartbeat_loop::run_heartbeat_loop};

fn remote_target() -> ActorPath {
  ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse")
}

fn local_watcher() -> ActorPath {
  ActorPath::root().child("user").child("me")
}

#[tokio::test(flavor = "current_thread")]
async fn watcher_actor_processes_watch_command_and_emits_initial_heartbeat_effect() {
  let (effect_tx, mut effect_rx) = mpsc::unbounded_channel::<WatcherEffect>();
  let actor = WatcherActor::with_default_detectors(effect_tx);
  let (handle, task) = actor.spawn();

  handle.submit(WatcherCommand::Watch { target: remote_target(), watcher: local_watcher() }).unwrap();

  let effect = tokio::time::timeout(Duration::from_secs(1), effect_rx.recv())
    .await
    .unwrap()
    .expect("watch should produce a SendHeartbeat effect");
  assert!(matches!(effect, WatcherEffect::SendHeartbeat { .. }));

  drop(handle);
  task.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn watcher_actor_handles_heartbeat_received_then_tick_without_terminating() {
  let (effect_tx, mut effect_rx) = mpsc::unbounded_channel::<WatcherEffect>();
  let actor = WatcherActor::with_default_detectors(effect_tx);
  let (handle, task) = actor.spawn();

  // Watch a remote target so the actor knows about its node.
  handle.submit(WatcherCommand::Watch { target: remote_target(), watcher: local_watcher() }).unwrap();
  // Drain the initial SendHeartbeat effect.
  drop(tokio::time::timeout(Duration::from_secs(1), effect_rx.recv()).await);

  // Send a few heartbeats from the peer.
  let node = Address::new("remote-sys", "10.0.0.1", 2552);
  for i in 0..5_u64 {
    handle.submit(WatcherCommand::HeartbeatReceived { from: node.clone(), now: i * 100 }).unwrap();
  }

  // Tick at a time well before any timeout would fire — must NOT see a
  // NotifyTerminated effect.
  handle.submit(WatcherCommand::HeartbeatTick { now: 600 }).unwrap();
  // Allow the actor to process the tick and produce its periodic
  // SendHeartbeat effect.
  let mut saw_terminated = false;
  for _ in 0..3 {
    if let Ok(Some(effect)) = tokio::time::timeout(Duration::from_millis(50), effect_rx.recv()).await {
      if matches!(effect, WatcherEffect::NotifyTerminated { .. }) {
        saw_terminated = true;
        break;
      }
    } else {
      break;
    }
  }
  assert!(!saw_terminated, "no termination should fire while heartbeats are healthy");

  drop(handle);
  task.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn watcher_actor_forwards_heartbeat_response_rewatch_effect() {
  let (effect_tx, mut effect_rx) = mpsc::unbounded_channel::<WatcherEffect>();
  let actor = WatcherActor::with_default_detectors(effect_tx);
  let (handle, task) = actor.spawn();

  handle.submit(WatcherCommand::Watch { target: remote_target(), watcher: local_watcher() }).unwrap();
  drop(tokio::time::timeout(Duration::from_secs(1), effect_rx.recv()).await);

  let node = Address::new("remote-sys", "10.0.0.1", 2552);
  handle.submit(WatcherCommand::HeartbeatResponseReceived { from: node.clone(), uid: 42, now: 100 }).unwrap();

  let effect = tokio::time::timeout(Duration::from_secs(1), effect_rx.recv())
    .await
    .unwrap()
    .expect("heartbeat response with initial UID should produce a rewatch effect");
  assert!(matches!(
    effect,
    WatcherEffect::RewatchRemoteTargets {
      node: effect_node,
      ..
    } if effect_node == node
  ));

  drop(handle);
  task.await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn heartbeat_loop_delivers_ticks_at_configured_interval() {
  let (effect_tx, mut effect_rx) = mpsc::unbounded_channel::<WatcherEffect>();
  let actor = WatcherActor::with_default_detectors(effect_tx);
  let (handle, actor_task) = actor.spawn();

  // Watch a remote target so the actor has a node to issue periodic
  // SendHeartbeat effects against.
  handle.submit(WatcherCommand::Watch { target: remote_target(), watcher: local_watcher() }).unwrap();
  // Drain the initial SendHeartbeat caused by the Watch command.
  drop(tokio::time::timeout(Duration::from_millis(50), effect_rx.recv()).await);

  let loop_handle = handle.clone();
  let loop_task = tokio::spawn(async move { run_heartbeat_loop(loop_handle, Duration::from_millis(20)).await });

  // Collect SendHeartbeat effects produced by periodic ticks.
  let mut tick_effects = 0_u32;
  let deadline = Instant::now() + Duration::from_millis(150);
  while Instant::now() < deadline {
    match tokio::time::timeout(Duration::from_millis(30), effect_rx.recv()).await {
      | Ok(Some(WatcherEffect::SendHeartbeat { .. })) => tick_effects += 1,
      | Ok(Some(_)) => {},
      | _ => {},
    }
  }

  drop(handle);
  loop_task.abort();
  drop(loop_task.await);
  drop(actor_task.await);

  assert!(tick_effects >= 2, "expected at least 2 periodic SendHeartbeat effects, got {tick_effects}");
}
