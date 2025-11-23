use alloc::{string::ToString, vec::Vec};
use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use super::SystemState;
use crate::core::{
  actor_prim::{
    Actor, ActorCell, ActorContextGeneric,
    actor_path::{ActorPath, ActorPathScheme, ActorUid, GuardianKind as PathGuardianKind, PathResolutionError},
    actor_ref::ActorRefGeneric,
  },
  error::ActorError,
  event_stream::{EventStream, EventStreamEvent, EventStreamSubscriber},
  messaging::{AnyMessage, AnyMessageViewGeneric},
  props::Props,
  system::{ActorSystemConfig, AuthorityState, RegisterExtraTopLevelError, RemotingConfig},
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
  assert!(!future.is_ready());
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
  let state = ArcShared::new(SystemState::new());
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

  state.remove_cell(&child_pid);
  assert!(state.cell(&child_pid).is_none());
}

#[test]
fn system_state_remove_cell_reserves_uid() {
  let state = SystemState::new();
  let pid = state.allocate_pid();
  let path = ActorPath::root().child("reserved").with_uid(ActorUid::new(777));

  {
    let mut registry = state.actor_path_registry().lock();
    registry.register(pid, &path);
  }

  let _ = state.remove_cell(&pid);

  let mut registry = state.actor_path_registry().lock();
  let now = state.monotonic_now().as_secs();
  let result = registry.reserve_uid(&path, ActorUid::new(888), now, None);
  assert!(matches!(result, Err(PathResolutionError::UidReserved { .. })));
}

#[test]
fn system_state_registers_canonical_uri_with_config() {
  let state = ArcShared::new(SystemState::new());
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

  let registry = state.actor_path_registry().lock();
  let canonical = registry.canonical_uri(&child_pid).expect("canonical uri");
  assert!(canonical.starts_with("fraktor.tcp://fraktor-system@localhost:2552"));
  assert!(canonical.ends_with("/user/worker"));
}

#[test]
fn system_state_prefers_advertise_authority_for_canonical_path() {
  let state = ArcShared::new(SystemState::new());
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
  let state = ArcShared::new(SystemState::new());
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
  assert!(state.actor_path_registry().lock().get(&child_pid).is_none());
  let local = state.actor_path(&child_pid).expect("local path");
  assert_eq!(local.to_relative_string(), "/user/worker");
  assert!(state.canonical_authority_components().is_none());
}
#[test]
fn system_state_honors_default_guardian_config() {
  let state = ArcShared::new(SystemState::new());
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

  let registry = state.actor_path_registry().lock();
  let canonical = registry.canonical_uri(&child_pid).expect("canonical uri");
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
  use fraktor_utils_rs::core::sync::ArcShared;

  use crate::core::futures::ActorFuture;

  let state = SystemState::new();
  let future = ArcShared::new(ActorFuture::<AnyMessage>::new());
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
  let subscriber_impl = ArcShared::new(RemoteEventRecorder::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStream::subscribe_arc(&stream, &subscriber);

  state.remote_authority_set_quarantine("node:2552", Some(Duration::from_secs(0)));
  state.poll_remote_authorities();

  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::RemoteAuthority(remote)
    if remote.authority() == "node:2552" && matches!(remote.state(), AuthorityState::Quarantine { .. }))));
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::RemoteAuthority(remote)
    if remote.authority() == "node:2552" && matches!(remote.state(), AuthorityState::Unresolved))));

  // InvalidAssociation による隔離通知
  state.remote_authority_handle_invalid_association("node:2552", Some(Duration::from_secs(5)));
  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::RemoteAuthority(remote)
    if remote.authority() == "node:2552" && matches!(remote.state(), AuthorityState::Quarantine { .. }))));

  // 手動解除と接続通知
  state.remote_authority_manual_override_to_connected("node:2552");
  let events = subscriber_impl.events();
  assert!(events.iter().any(|event| matches!(event, EventStreamEvent::RemoteAuthority(remote)
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

struct RestartProbeActor;

struct RemoteEventRecorder {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl RemoteEventRecorder {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<EventStreamEvent<NoStdToolbox>> {
    self.events.lock().clone()
  }
}

impl Default for RemoteEventRecorder {
  fn default() -> Self {
    Self::new()
  }
}

impl EventStreamSubscriber<NoStdToolbox> for RemoteEventRecorder {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
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
  let state = ArcShared::new(SystemState::new());
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

  state.remove_cell(&child_pid);
  let error = ActorError::recoverable("boom");
  state.handle_failure(child_pid, Some(parent_pid), &error);

  assert!(state.cell(&parent_pid).is_none());
}
