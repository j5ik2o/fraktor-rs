use std::time::{Duration, Instant};

use fraktor_actor_adaptor_std_rs::{
  system::{create_noop_actor_system, std_actor_system_config},
  tick_driver::TestTickDriver,
};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_path::{ActorPath, ActorPathParser},
    actor_ref::ActorRef,
    actor_ref_provider::LocalActorRefProviderInstaller,
    messaging::system_message::SystemMessage,
  },
  system::ActorSystem,
};
use fraktor_remote_core_rs::{address::Address, extension::RemoteEvent, watcher::WatcherEffect, wire::ControlPdu};
use tokio::{sync::mpsc, time::timeout};

use super::{
  apply_effects, default_detector_factory, notify_local_watchers, run_watcher_task, send_heartbeat,
  send_redelivery_tick, send_system_envelope,
};

fn local_address() -> Address {
  Address::new("local-sys", "127.0.0.1", 2551)
}

fn remote_address() -> Address {
  Address::new("remote-sys", "10.0.0.1", 2552)
}

fn remote_path(name: &str) -> ActorPath {
  ActorPathParser::parse(&alloc::format!("fraktor.tcp://remote-sys@10.0.0.1:2552/user/{name}")).expect("parse")
}

fn local_path(name: &str) -> ActorPath {
  ActorPath::root().child("user").child(name)
}

fn local_actor_system() -> ActorSystem {
  let config = std_actor_system_config(TestTickDriver::default())
    .with_actor_ref_provider_installer(LocalActorRefProviderInstaller::default());
  ActorSystem::create_with_noop_guardian(config).expect("actor system should build")
}

fn user_guardian_path(system: &ActorSystem) -> ActorPath {
  system.user_guardian_ref().path().expect("user guardian path")
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn run_watcher_task_returns_when_command_channel_closes() {
  let (command_tx, command_rx) = mpsc::channel(1);
  let (event_tx, _event_rx) = mpsc::channel(8);
  drop(command_tx);

  timeout(
    Duration::from_secs(1),
    run_watcher_task(
      command_rx,
      event_tx,
      create_noop_actor_system(),
      local_address(),
      Instant::now(),
      Duration::from_millis(10),
    ),
  )
  .await
  .expect("watcher task should exit when command channel closes");
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn apply_effects_emits_remote_events_for_watch_heartbeat_and_rewatch() {
  let (event_tx, mut event_rx) = mpsc::channel(8);
  let system = local_actor_system();
  let target = remote_path("target");
  let watcher = local_path("watcher");
  let remote = remote_address();
  let local_target_path = user_guardian_path(&system);
  let local_watcher_path = local_target_path.clone();

  apply_effects(
    alloc::vec![
      WatcherEffect::SendWatch { target: target.clone(), watcher: watcher.clone() },
      WatcherEffect::SendUnwatch { target: target.clone(), watcher: watcher.clone() },
      WatcherEffect::SendHeartbeat { to: remote.clone() },
      WatcherEffect::NotifyTerminated { target: local_target_path, watchers: alloc::vec![local_watcher_path] },
      WatcherEffect::NotifyQuarantined { node: remote.clone() },
      WatcherEffect::RewatchRemoteTargets { node: remote.clone(), watches: alloc::vec![(target, watcher)] },
    ],
    &event_tx,
    &system,
    &local_address(),
    Instant::now(),
    42,
  )
  .await;

  let mut events = alloc::vec![];
  while let Ok(event) = event_rx.try_recv() {
    events.push(event);
  }
  assert_eq!(events.len(), 5);
  assert!(events.iter().any(|event| matches!(
    event,
    RemoteEvent::OutboundControl {
      remote: event_remote,
      pdu: ControlPdu::Heartbeat { authority },
      now_ms: 42,
    } if event_remote == &remote && authority == "local-sys@127.0.0.1:2551"
  )));
  assert!(events.iter().any(|event| matches!(
    event,
    RemoteEvent::RedeliveryTimerFired {
      authority,
      now_ms: 42,
    } if authority.authority() == "remote-sys@10.0.0.1:2552"
  )));
  assert_eq!(events.iter().filter(|event| matches!(event, RemoteEvent::OutboundEnqueued { .. })).count(), 3);
}

#[test]
fn notify_local_watchers_returns_when_target_cannot_be_resolved() {
  let system = local_actor_system();

  notify_local_watchers(&system, local_path("missing-target"), alloc::vec![local_path("watcher")]);
}

#[test]
fn notify_local_watchers_skips_unresolved_watcher() {
  let system = local_actor_system();
  let target_path = user_guardian_path(&system);
  assert!(system.resolve_actor_ref(target_path.clone()).is_ok());

  notify_local_watchers(&system, target_path, alloc::vec![local_path("missing-watcher")]);
}

#[test]
fn notify_local_watchers_logs_when_send_fails() {
  let system = local_actor_system();
  let target_path = user_guardian_path(&system);
  let failing_watcher = ActorRef::null();
  let failing_watcher_pid = failing_watcher.pid();
  let _name = system.state().register_temp_actor(failing_watcher);
  let failing_watcher_path = system.state().canonical_actor_path(&failing_watcher_pid).expect("failing watcher path");

  notify_local_watchers(&system, target_path, alloc::vec![failing_watcher_path]);
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn watcher_send_helpers_log_and_return_when_event_receiver_is_closed() {
  let (event_tx, event_rx) = mpsc::channel(1);
  drop(event_rx);

  send_heartbeat(&event_tx, &local_address(), remote_address(), 10).await;
  send_redelivery_tick(&event_tx, remote_address(), 11).await;
  send_system_envelope(&event_tx, remote_path("closed"), None, SystemMessage::Watch(Pid::new(0, 0)), Instant::now())
    .await;
}

#[tokio::test(flavor = "current_thread", start_paused = false)]
async fn send_system_envelope_does_not_enqueue_event_for_local_recipient() {
  let (event_tx, mut event_rx) = mpsc::channel(1);

  send_system_envelope(
    &event_tx,
    local_path("not-remote"),
    None,
    SystemMessage::Unwatch(Pid::new(0, 0)),
    Instant::now(),
  )
  .await;

  assert!(event_rx.try_recv().is_err());
}

#[test]
fn default_detector_factory_creates_available_detector() {
  let detector = default_detector_factory(&remote_address());

  assert!(detector.is_available(0));
}
