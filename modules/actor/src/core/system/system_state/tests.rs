use alloc::{string::ToString, vec::Vec};
use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::{ArcShared, SharedAccess},
};

use super::SystemState;
use crate::core::{
  actor_prim::{
    Actor, ActorCell, ActorContextGeneric,
    actor_path::{ActorPath, ActorPathScheme, ActorUid, GuardianKind as PathGuardianKind, PathResolutionError},
    actor_ref::ActorRefGeneric,
  },
  dispatcher::{DispatchError, DispatchExecutor, DispatchSharedGeneric, DispatcherConfig},
  error::ActorError,
  event_stream::{EventStream, EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  mailbox::MailboxMessage,
  messaging::{AnyMessage, AnyMessageViewGeneric, FailurePayload, SystemMessage},
  props::Props,
  system::{
    ActorSystemConfig, AuthorityState, GuardianKind, RegisterExtraTopLevelError, RemotingConfig, SystemStateShared,
    booting_state::BootingSystemStateGeneric,
  },
};

#[test]
fn system_state_new() {
  let state = SystemState::new();
  assert!(!state.is_terminated());
  assert_eq!(state.dead_letters().len(), 0);
}

#[test]
fn system_state_default() {
  let state = SystemState::default();
  assert!(!state.is_terminated());
}

#[test]
fn system_state_allocate_pid() {
  let state = SystemState::new();
  let pid1 = state.allocate_pid();
  let pid2 = state.allocate_pid();
  assert_ne!(pid1.value(), pid2.value());
}

#[test]
fn system_state_monotonic_now() {
  let state = SystemState::new();
  let now1 = state.monotonic_now();
  let now2 = state.monotonic_now();
  assert!(now2 > now1);
}

#[test]
fn system_state_event_stream() {
  let state = SystemState::new();
  let stream = state.event_stream();
  let _ = stream;
}

#[test]
fn system_state_termination_future() {
  let state = SystemState::new();
  let future = state.termination_future();
  assert!(!future.with_read(|af| af.is_ready()));
}

#[test]
fn system_state_mark_terminated() {
  let state = SystemState::new();
  assert!(!state.is_terminated());
  state.mark_terminated();
  assert!(state.is_terminated());
}

#[test]
fn system_state_register_and_remove_cell() {
  let state = SystemStateShared::new(SystemState::new());
  let root_pid = state.allocate_pid();
  let child_pid = state.allocate_pid();
  let props = Props::from_fn(|| RestartProbeActor);
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("create actor cell");
  state.register_cell(root);
  let child = ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(child.clone());

  assert!(state.cell(&child_pid).is_some());
  let path = state.actor_path(&child_pid).expect("path");
  assert_eq!(path.to_string(), "/user/worker");

  let _ = state.remove_cell(&child_pid);
  assert!(state.cell(&child_pid).is_none());
}

#[test]
fn system_state_remove_cell_reserves_uid() {
  let state = SystemState::new();
  let pid = state.allocate_pid();
  let path = ActorPath::root().child("reserved").with_uid(ActorUid::new(777));

  state.actor_path_registry().with_write(|registry| {
    registry.register(pid, &path);
  });

  let _ = state.remove_cell(&pid);

  let now = state.monotonic_now().as_secs();
  let result =
    state.actor_path_registry().with_write(|registry| registry.reserve_uid(&path, ActorUid::new(888), now, None));
  assert!(matches!(result, Err(PathResolutionError::UidReserved { .. })));
}

#[test]
fn system_state_registers_canonical_uri_with_config() {
  let state = SystemStateShared::new(SystemState::new());
  let remoting = RemotingConfig::default().with_canonical_host("localhost").with_canonical_port(2552);
  let config = ActorSystemConfig::default().with_system_name("fraktor-system").with_remoting_config(remoting);
  state.apply_actor_system_config(&config);

  let props = Props::from_fn(|| RestartProbeActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props).expect("worker");
  state.register_cell(child);

  let canonical = state.with_actor_path_registry(|registry| {
    registry.with_read(|r| r.canonical_uri(&child_pid).expect("canonical uri").to_string())
  });
  assert!(canonical.starts_with("fraktor.tcp://fraktor-system@localhost:2552"));
  assert!(canonical.ends_with("/user/worker"));
}

#[test]
fn system_state_prefers_advertise_authority_for_canonical_path() {
  let state = SystemStateShared::new(SystemState::new());
  let remoting = RemotingConfig::default().with_canonical_host("public.example.com").with_canonical_port(4100);
  let config = ActorSystemConfig::default().with_system_name("fraktor-system").with_remoting_config(remoting);
  state.apply_actor_system_config(&config);

  let props = Props::from_fn(|| RestartProbeActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props).expect("worker");
  state.register_cell(child);

  let canonical = state.canonical_actor_path(&child_pid).expect("canonical path");
  assert_eq!(canonical.parts().scheme(), ActorPathScheme::FraktorTcp);
  assert_eq!(canonical.parts().authority_endpoint(), Some("public.example.com:4100".to_string()));
  assert!(canonical.to_canonical_uri().contains("public.example.com:4100"));
}

#[test]
fn system_state_refuses_canonical_without_port() {
  let state = SystemStateShared::new(SystemState::new());
  let remoting = RemotingConfig::default().with_canonical_host("missing-port.example");
  let config = ActorSystemConfig::default().with_system_name("fraktor-system").with_remoting_config(remoting);
  state.apply_actor_system_config(&config);

  let props = Props::from_fn(|| RestartProbeActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props).expect("worker");
  state.register_cell(child);

  assert!(state.canonical_actor_path(&child_pid).is_none());
  assert!(state.with_actor_path_registry(|registry| registry.with_read(|r| r.get(&child_pid).is_none())));
  let local = state.actor_path(&child_pid).expect("local path");
  assert_eq!(local.to_relative_string(), "/user/worker");
  assert!(state.canonical_authority_components().is_none());
}
#[test]
fn system_state_honors_default_guardian_config() {
  let state = SystemStateShared::new(SystemState::new());
  let config =
    ActorSystemConfig::default().with_system_name("sys-guardian").with_default_guardian(PathGuardianKind::System);
  state.apply_actor_system_config(&config);

  let props = Props::from_fn(|| RestartProbeActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "logger".to_string(), &props).expect("logger");
  state.register_cell(child);

  let canonical = state.with_actor_path_registry(|registry| {
    registry.with_read(|r| r.canonical_uri(&child_pid).expect("canonical uri").to_string())
  });
  assert!(canonical.contains("/system/logger"), "canonical: {}", canonical);
}

#[test]
fn system_state_assign_name_with_hint() {
  let state = SystemState::new();
  let pid = state.allocate_pid();

  let result = state.assign_name(None, Some("test-actor"), pid);
  assert!(result.is_ok());
  let name = result.unwrap();
  assert_eq!(name, "test-actor");
}

#[test]
fn system_state_assign_name_without_hint() {
  let state = SystemState::new();
  let pid = state.allocate_pid();

  let result = state.assign_name(None, None, pid);
  assert!(result.is_ok());
  let name = result.unwrap();

  assert!(!name.is_empty());
}

#[test]
fn system_state_release_name() {
  let state = SystemState::new();
  let pid = state.allocate_pid();

  let _name = state.assign_name(None, Some("test-actor"), pid).unwrap();
  state.release_name(None, "test-actor");
}

#[test]
fn system_state_user_guardian_pid() {
  let state = SystemState::new();
  assert!(state.user_guardian_pid().is_none());
}

#[test]
fn system_state_child_pids() {
  let state = SystemState::new();
  let parent_pid = state.allocate_pid();

  let children = state.child_pids(parent_pid);
  assert_eq!(children.len(), 0);
}

#[test]
fn system_state_deadletters() {
  let state = SystemState::new();
  let dead_letters = state.dead_letters();
  assert_eq!(dead_letters.len(), 0);
}

#[test]
fn system_state_register_ask_future() {
  use crate::core::futures::ActorFutureSharedGeneric;

  let state = SystemState::new();
  let future = ActorFutureSharedGeneric::<AnyMessage, NoStdToolbox>::new();
  state.register_ask_future(future.clone());

  let ready = state.drain_ready_ask_futures();
  assert_eq!(ready.len(), 0);
}

#[test]
fn system_state_publish_event() {
  use alloc::string::String;
  use core::time::Duration;

  use crate::core::{
    event_stream::EventStreamEvent,
    logging::{LogEvent, LogLevel},
  };

  let state = SystemState::new();
  let log_event = LogEvent::new(LogLevel::Info, String::from("test"), Duration::from_millis(1), None);
  let event = EventStreamEvent::Log(log_event);

  state.publish_event(&event);
}

#[test]
fn system_state_emit_log() {
  use alloc::string::String;

  let state = SystemState::new();
  let pid = state.allocate_pid();

  state.emit_log(crate::core::logging::LogLevel::Info, String::from("test message"), Some(pid));
  state.emit_log(crate::core::logging::LogLevel::Error, String::from("error message"), None);
}

#[test]
fn system_state_clear_guardian() {
  let state = SystemState::new();
  let pid = state.allocate_pid();

  let cleared = state.clear_guardian(pid);
  assert!(cleared.is_none());
}

#[test]
fn system_state_user_guardian() {
  let state = SystemState::new();
  assert!(state.user_guardian().is_none());
}

#[test]
fn system_state_register_extra_top_level_success() {
  let state = SystemState::new();
  let actor = ActorRefGeneric::null();
  assert!(state.register_extra_top_level("metrics", actor.clone()).is_ok());
  assert!(state.extra_top_level("metrics").is_some());
}

#[test]
fn system_state_register_extra_top_level_errors() {
  let state = SystemState::new();
  let actor = ActorRefGeneric::null();
  let reserved = state.register_extra_top_level("user", actor.clone());
  assert!(matches!(reserved, Err(RegisterExtraTopLevelError::ReservedName(_))));
  state.mark_root_started();
  let started = state.register_extra_top_level("custom", actor);
  assert!(matches!(started, Err(RegisterExtraTopLevelError::AlreadyStarted)));
}

#[test]
fn system_state_temp_actor_round_trip() {
  let state = SystemState::new();
  let actor = ActorRefGeneric::null();
  let name = state.register_temp_actor(actor.clone());
  assert!(state.temp_actor(&name).is_some());
  state.unregister_temp_actor(&name);
  assert!(state.temp_actor(&name).is_none());
}

#[test]
fn system_state_remote_authority_events() {
  let state = SystemState::new();
  let stream = state.event_stream();
  let events_shared = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RemoteEventRecorder::new(events_shared.clone()));
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  state.remote_authority_set_quarantine("node:2552", Some(Duration::from_secs(0)));
  state.poll_remote_authorities();

  let events_snapshot = events_shared.lock().clone();
  assert!(events_snapshot.iter().any(|event| matches!(event, EventStreamEvent::RemoteAuthority(remote)
    if remote.authority() == "node:2552" && matches!(remote.state(), AuthorityState::Quarantine { .. }))));
  assert!(events_snapshot.iter().any(|event| matches!(event, EventStreamEvent::RemoteAuthority(remote)
    if remote.authority() == "node:2552" && matches!(remote.state(), AuthorityState::Unresolved))));

  // InvalidAssociation による隔離通知
  state.remote_authority_handle_invalid_association("node:2552", Some(Duration::from_secs(5)));
  let events_snapshot = events_shared.lock().clone();
  assert!(events_snapshot.iter().any(|event| matches!(event, EventStreamEvent::RemoteAuthority(remote)
    if remote.authority() == "node:2552" && matches!(remote.state(), AuthorityState::Quarantine { .. }))));

  // 手動解除と接続通知
  state.remote_authority_manual_override_to_connected("node:2552");
  let events_snapshot = events_shared.lock().clone();
  assert!(events_snapshot.iter().any(|event| matches!(event, EventStreamEvent::RemoteAuthority(remote)
    if remote.authority() == "node:2552" && matches!(remote.state(), AuthorityState::Connected))));
}

#[test]
fn system_state_send_system_message_to_nonexistent_actor() {
  use crate::core::messaging::SystemMessage;

  let state = SystemState::new();
  let pid = state.allocate_pid();

  let result = state.send_system_message(pid, SystemMessage::Stop);
  assert!(result.is_err());
}

#[test]
fn system_state_record_send_error() {
  use crate::core::error::SendError;

  let state = SystemState::new();
  let error = SendError::closed(AnyMessage::new(42_u32));

  state.record_send_error(None, &error);
  state.record_send_error(Some(state.allocate_pid()), &error);
}

#[test]
fn guardian_cell_via_cells_returns_none_when_missing() {
  let state = SystemStateShared::new(SystemState::new());
  let user_pid = state.allocate_pid();

  state.register_guardian_pid(GuardianKind::User, user_pid);

  assert!(state.user_guardian().is_none());
  assert_eq!(state.user_guardian_pid(), Some(user_pid));
}

#[test]
fn booting_into_running_requires_all_guardians() {
  let state = SystemStateShared::new(SystemState::new());
  let booting = BootingSystemStateGeneric::new(state.clone());

  let root_pid = state.allocate_pid();
  let system_pid = state.allocate_pid();
  let user_pid = state.allocate_pid();

  let props = Props::from_fn(|| RestartProbeActor);
  let root_cell =
    ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root cell created");
  let system_cell =
    ActorCell::create(state.clone(), system_pid, Some(root_pid), "system".to_string(), &props).expect("system cell");
  let user_cell =
    ActorCell::create(state.clone(), user_pid, Some(root_pid), "user".to_string(), &props).expect("user cell");

  state.register_cell(root_cell);
  state.register_cell(system_cell.clone());
  state.register_cell(user_cell.clone());

  booting.register_guardian(GuardianKind::Root, root_pid);
  booting.register_guardian(GuardianKind::System, system_pid);
  booting.register_guardian(GuardianKind::User, user_pid);

  let running = booting.into_running().expect("running state");
  assert_eq!(running.guardian_pid(GuardianKind::User), user_pid);
  assert!(running.guardian_cell(GuardianKind::User).is_some());
  assert!(running.guardian_cell(GuardianKind::System).is_some());
}

#[test]
fn booting_into_running_fails_when_guardian_missing() {
  let state = SystemStateShared::new(SystemState::new());
  let booting = BootingSystemStateGeneric::new(state.clone());

  let root_pid = state.allocate_pid();
  let system_pid = state.allocate_pid();
  booting.register_guardian(GuardianKind::Root, root_pid);
  booting.register_guardian(GuardianKind::System, system_pid);

  let result = booting.into_running();
  assert!(matches!(result, Err(crate::core::spawn::SpawnError::SystemNotBootstrapped)));
}

#[test]
fn watch_on_missing_guardian_sends_terminated_to_watcher() {
  let state = SystemStateShared::new(SystemState::new());
  let watcher_pid = state.allocate_pid();
  let target_pid = state.allocate_pid();

  let noop_dispatcher = DispatcherConfig::from_executor(Box::new(NoopExecutor));
  let props = Props::from_fn(|| RestartProbeActor).with_dispatcher(noop_dispatcher);
  let watcher_cell =
    ActorCell::create(state.clone(), watcher_pid, None, "watcher".to_string(), &props).expect("watcher cell");
  state.register_cell(watcher_cell);

  state.send_system_message(target_pid, SystemMessage::Watch(watcher_pid)).expect("watch send ok");

  let mailbox_snapshot = state.cell(&watcher_pid).expect("watcher cell").mailbox();
  assert_eq!(mailbox_snapshot.system_len(), 1);
  let dequeued = mailbox_snapshot.dequeue().expect("dequeue system");
  match dequeued {
    | MailboxMessage::System(SystemMessage::Terminated(pid)) => assert_eq!(pid, target_pid),
    | other => panic!("unexpected mailbox message: {:?}", other),
  }
}

#[test]
fn termination_future_completes_after_root_marked_terminated() {
  let state = SystemStateShared::new(SystemState::new());
  let root_pid = state.allocate_pid();
  state.register_guardian_pid(GuardianKind::Root, root_pid);

  assert!(!state.termination_future().with_read(|f| f.is_ready()));
  let _ = state.clear_guardian(root_pid);
  state.mark_terminated();

  assert!(state.termination_future().with_read(|f| f.is_ready()));
}

#[test]
fn system_state_logs_failure_with_pid_origin() {
  use core::time::Duration;

  let state = SystemState::new();
  let events_shared: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>> =
    ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(LogRecorder::new(events_shared.clone()));
  let _subscription = EventStream::subscribe_arc(&state.event_stream(), &subscriber);

  let pid = state.allocate_pid();
  let payload = FailurePayload::from_error(pid, &ActorError::fatal("boom"), None, Duration::from_millis(1));

  state.report_failure(payload);

  let events_snapshot = events_shared.lock().clone();
  let log_event = events_snapshot.iter().find_map(|event| match event {
    | EventStreamEvent::Log(log) => Some(log.clone()),
    | _ => None,
  });

  let log_event = log_event.expect("log event should be published");
  assert_eq!(log_event.origin(), Some(pid));
  assert!(log_event.message().contains("failed"));
}

struct RestartProbeActor;

struct RemoteEventRecorder {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl RemoteEventRecorder {
  fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>) -> Self {
    Self { events }
  }
}

impl Default for RemoteEventRecorder {
  fn default() -> Self {
    Self::new(ArcShared::new(NoStdMutex::new(Vec::new())))
  }
}

impl EventStreamSubscriber<NoStdToolbox> for RemoteEventRecorder {
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

struct NoopExecutor;

impl DispatchExecutor<NoStdToolbox> for NoopExecutor {
  fn execute(&mut self, _dispatcher: DispatchSharedGeneric<NoStdToolbox>) -> Result<(), DispatchError> {
    Ok(())
  }

  fn supports_blocking(&self) -> bool {
    false
  }
}

struct LogRecorder {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl LogRecorder {
  fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>) -> Self {
    Self { events }
  }
}

impl Default for LogRecorder {
  fn default() -> Self {
    Self::new(ArcShared::new(NoStdMutex::new(Vec::new())))
  }
}

impl EventStreamSubscriber<NoStdToolbox> for LogRecorder {
  fn on_event(&mut self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

impl Actor for RestartProbeActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn recreate_send_failure_escalates_and_stops_parent() {
  let state = SystemStateShared::new(SystemState::new());
  let parent_pid = state.allocate_pid();
  let parent_props = Props::from_fn(|| RestartProbeActor);
  let parent =
    ActorCell::create(state.clone(), parent_pid, None, "parent".to_string(), &parent_props).expect("create actor cell");
  state.register_cell(parent.clone());

  let child_pid = state.allocate_pid();
  let child_props = Props::from_fn(|| RestartProbeActor);
  let child = ActorCell::create(state.clone(), child_pid, Some(parent_pid), "child".to_string(), &child_props)
    .expect("create actor cell");
  state.register_cell(child.clone());
  state.register_child(parent_pid, child_pid);

  let _ = state.remove_cell(&child_pid);
  let error = ActorError::recoverable("boom");
  state.handle_failure(child_pid, Some(parent_pid), &error);

  assert!(state.cell(&parent_pid).is_none());
}
