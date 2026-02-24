#![cfg(any(test, feature = "test-support"))]

use alloc::boxed::Box;

use fraktor_actor_rs::core::{
  actor::{
    Actor, ActorContextGeneric, Pid,
    actor_path::{ActorPathParts, GuardianKind},
  },
  error::ActorError,
  messaging::AnyMessageViewGeneric,
  props::PropsGeneric,
  scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  system::{ActorSystemConfig, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, sync_mutex_family::SyncMutexFamily},
  sync::ArcShared,
};

use super::RemoteWatcherDaemon;
use crate::core::{
  failure_detector::FailureDetector,
  remoting_extension::{
    RemotingControl, RemotingControlHandle, RemotingControlShared, RemotingError, RemotingExtensionConfig,
  },
  transport::{LoopbackTransport, RemoteTransport, RemoteTransportShared, TransportBind},
  watcher::{command::RemoteWatcherCommand, heartbeat::Heartbeat, heartbeat_rsp::HeartbeatRsp},
};

struct NoopActor;

impl Actor<NoStdToolbox> for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystemGeneric<NoStdToolbox> {
  let props = PropsGeneric::from_fn(|| NoopActor).with_name("remote-watcher-daemon-tests");
  let config = ActorSystemConfig::default().with_tick_driver(TickDriverConfig::manual(ManualTestDriver::new()));
  ActorSystemGeneric::new_with_config(&props, &config).expect("system")
}

fn build_control(system: &ActorSystemGeneric<NoStdToolbox>) -> RemotingControlShared<NoStdToolbox> {
  let handle = RemotingControlHandle::new(system.clone(), RemotingExtensionConfig::default());
  let control: RemotingControlShared<NoStdToolbox> =
    ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(handle));
  let mut transport = LoopbackTransport::<NoStdToolbox>::default();
  transport.spawn_listener(&TransportBind::new("127.0.0.1", Some(4100))).expect("bind 127.0.0.1:4100");
  control.lock().register_remote_transport_shared(RemoteTransportShared::new(Box::new(transport)));
  control.lock().start().expect("control start");
  control
}

fn build_control_without_start(system: &ActorSystemGeneric<NoStdToolbox>) -> RemotingControlShared<NoStdToolbox> {
  let handle = RemotingControlHandle::new(system.clone(), RemotingExtensionConfig::default());
  ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(handle))
}

fn remote_target() -> ActorPathParts {
  ActorPathParts::with_authority("remote-app", Some(("127.0.0.1", 4100))).with_guardian(GuardianKind::User)
}

#[test]
fn watch_and_unwatch_update_local_registry() {
  let system = build_system();
  let mut daemon: RemoteWatcherDaemon<NoStdToolbox> = RemoteWatcherDaemon::new(build_control(&system));
  let target = remote_target();
  let watcher = Pid::new(42, 0);

  daemon.handle_command(&RemoteWatcherCommand::Watch { target: target.clone(), watcher }, 100).expect("watch command");
  assert_eq!(daemon.watchers.len(), 1);

  daemon.handle_command(&RemoteWatcherCommand::Unwatch { target, watcher }, 150).expect("unwatch command");
  assert!(daemon.watchers.is_empty());
}

#[test]
fn watch_returns_error_when_remoting_control_is_not_started() {
  let system = build_system();
  let mut daemon: RemoteWatcherDaemon<NoStdToolbox> = RemoteWatcherDaemon::new(build_control_without_start(&system));
  let target = remote_target();
  let watcher = Pid::new(99, 0);

  let result = daemon.handle_command(&RemoteWatcherCommand::Watch { target, watcher }, 100);
  assert!(matches!(result, Err(RemotingError::NotStarted)));
}

#[test]
fn heartbeat_rsp_with_zero_uid_is_tracked_and_reaped_by_failure_detector() {
  let system = build_system();
  let mut daemon: RemoteWatcherDaemon<NoStdToolbox> = RemoteWatcherDaemon::new(build_control(&system));
  let authority = "127.0.0.1:4100";
  let target = remote_target();
  let watcher = Pid::new(7, 0);

  daemon.handle_command(&RemoteWatcherCommand::Watch { target, watcher }, 100).expect("watch command");
  daemon
    .handle_command(
      &RemoteWatcherCommand::HeartbeatRsp { heartbeat_rsp: HeartbeatRsp { authority: authority.into(), uid: 0 } },
      1_000,
    )
    .expect("heartbeat rsp command");
  daemon
    .handle_command(
      &RemoteWatcherCommand::HeartbeatRsp { heartbeat_rsp: HeartbeatRsp { authority: authority.into(), uid: 0 } },
      1_200,
    )
    .expect("heartbeat rsp command");

  daemon.handle_command(&RemoteWatcherCommand::ReapUnreachable, 4_000).expect("reap unreachable command");
  let detector = daemon.failure_detectors.get(authority).expect("failure detector");
  assert!(detector.is_monitoring());
}

#[test]
fn first_heartbeat_rsp_triggers_rewatch_for_watched_authority() {
  let system = build_system();
  let mut daemon: RemoteWatcherDaemon<NoStdToolbox> = RemoteWatcherDaemon::new(build_control(&system));
  let authority = "127.0.0.1:4100";
  let target = remote_target();
  let watcher = Pid::new(70, 0);

  daemon.handle_command(&RemoteWatcherCommand::Watch { target, watcher }, 100).expect("watch command");
  assert_eq!(daemon.rewatch_count, 0);

  daemon
    .handle_command(
      &RemoteWatcherCommand::HeartbeatRsp { heartbeat_rsp: HeartbeatRsp { authority: authority.into(), uid: 1 } },
      1_000,
    )
    .expect("heartbeat rsp command");
  assert_eq!(daemon.rewatch_count, 1);

  daemon
    .handle_command(
      &RemoteWatcherCommand::HeartbeatRsp { heartbeat_rsp: HeartbeatRsp { authority: authority.into(), uid: 1 } },
      1_100,
    )
    .expect("heartbeat rsp command");
  assert_eq!(daemon.rewatch_count, 1);
}

#[test]
fn heartbeat_rsp_uid_change_updates_cached_uid() {
  let system = build_system();
  let mut daemon: RemoteWatcherDaemon<NoStdToolbox> = RemoteWatcherDaemon::new(build_control(&system));
  let authority = "127.0.0.1:4100";
  let target = remote_target();
  let watcher = Pid::new(8, 0);

  daemon.handle_command(&RemoteWatcherCommand::Watch { target, watcher }, 100).expect("watch command");
  daemon
    .handle_command(
      &RemoteWatcherCommand::HeartbeatRsp { heartbeat_rsp: HeartbeatRsp { authority: authority.into(), uid: 1 } },
      1_000,
    )
    .expect("heartbeat rsp command");
  assert_eq!(daemon.authority_uids.get(authority).copied(), Some(1));

  daemon
    .handle_command(
      &RemoteWatcherCommand::HeartbeatRsp { heartbeat_rsp: HeartbeatRsp { authority: authority.into(), uid: 2 } },
      1_100,
    )
    .expect("heartbeat rsp command");
  assert_eq!(daemon.rewatch_count, 2);
  daemon.handle_command(&RemoteWatcherCommand::HeartbeatTick, 1_200).expect("heartbeat tick command");
  assert_eq!(daemon.authority_uids.get(authority).copied(), Some(2));
}

#[test]
fn heartbeat_probe_is_ignored_when_authority_is_not_watched() {
  let system = build_system();
  let mut daemon: RemoteWatcherDaemon<NoStdToolbox> = RemoteWatcherDaemon::new(build_control(&system));

  daemon
    .handle_command(
      &RemoteWatcherCommand::Heartbeat { heartbeat: Heartbeat { authority: "127.0.0.1:4999".into() } },
      1_000,
    )
    .expect("heartbeat command");
  daemon.handle_command(&RemoteWatcherCommand::ReapUnreachable, 4_000).expect("reap unreachable command");
  assert!(daemon.failure_detectors.is_empty());
}
