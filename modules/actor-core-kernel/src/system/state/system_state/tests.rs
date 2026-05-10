use alloc::{boxed::Box, string::ToString, vec, vec::Vec};
use core::{
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use super::SystemState;
use crate::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_path::{
      ActorPath, ActorPathParser, ActorPathScheme, ActorUid, GuardianKind as PathGuardianKind, PathResolutionError,
    },
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    error::{ActorError, SendError},
    messaging::{
      AnyMessage, AnyMessageView,
      system_message::{FailurePayload, SystemMessage},
    },
    props::Props,
    scheduler::{
      SchedulerConfig,
      tick_driver::{
        SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverKind, TickDriverProvision, TickDriverStopper,
        TickFeedHandle, next_tick_driver_id, tests::TestTickDriver,
      },
    },
    setup::ActorSystemConfig,
  },
  dispatch::dispatcher::{
    DefaultDispatcherFactory, DispatcherConfig, ExecuteError, Executor, MessageDispatcherFactory, TrampolineState,
  },
  event::stream::{EventStreamEvent, EventStreamSubscriber, tests::subscriber_handle},
  system::{
    RegisterExtraTopLevelError, TerminationSignal,
    guardian::GuardianKind,
    remote::RemotingConfig,
    state::{AuthorityState, SystemStateShared, system_state::LogLevel},
  },
};

impl SystemState {
  pub(crate) fn remove_cell(&mut self, pid: &Pid) {
    let reservation_source =
      self.actor_path_registry.get(pid).map(|handle| (handle.canonical_uri().to_string(), handle.uid()));

    if let Some((canonical, Some(uid))) = reservation_source
      && let Ok(actor_path) = ActorPathParser::parse(&canonical)
    {
      let now_secs = self.monotonic_now().as_secs();
      drop(self.actor_path_registry.reserve_uid(&actor_path, uid, now_secs, None));
    }

    self.actor_path_registry.unregister(pid);
    let _ = self.cells.with_write(|cells| cells.remove(pid));
  }

  #[must_use]
  pub(crate) fn child_pids(&self, parent: Pid) -> Vec<Pid> {
    self.cell(&parent).map_or_else(Vec::new, |cell| cell.children())
  }

  // ask_futures と temp_actors は SystemState 本体に実装しているため、テスト側の補助実装は不要
}

#[test]
fn system_state_build_from_config_starts_unterminated() {
  let state = build_state();
  assert!(!state.is_terminated());
  assert_eq!(state.dead_letters().len(), 0);
}

#[test]
fn system_state_build_from_config_provides_scheduler_and_tick_driver_bundle() {
  let state = build_state();
  let scheduler = state.scheduler();
  let resolution = scheduler.with_read(|s| s.config().resolution());
  let bundle = state.tick_driver_bundle();
  assert_eq!(bundle.resolution(), resolution);
}

#[test]
fn system_state_build_from_config_sets_non_zero_start_time_by_default() {
  let state = build_state();
  assert_ne!(state.start_time(), Duration::ZERO);
}

fn base_config() -> ActorSystemConfig {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler)
}

fn build_state() -> SystemState {
  SystemState::build_from_owned_config(base_config()).expect("state")
}

fn build_shared_state() -> SystemStateShared {
  SystemStateShared::new(build_state())
}

fn build_shared_state_with_noop_dispatcher() -> SystemStateShared {
  let config = base_config().with_dispatcher_factory("noop", noop_dispatcher_configurator());
  SystemStateShared::new(SystemState::build_from_owned_config(config).expect("state"))
}

struct StopCountingStopper {
  stop_calls: ArcShared<AtomicUsize>,
}

impl TickDriverStopper for StopCountingStopper {
  fn stop(self: Box<Self>) {
    self.stop_calls.fetch_add(1, Ordering::SeqCst);
  }
}

struct RecordingSystemMessageSender {
  messages: ArcShared<SpinSyncMutex<Vec<SystemMessage>>>,
}

impl RecordingSystemMessageSender {
  fn new() -> (ArcShared<SpinSyncMutex<Vec<SystemMessage>>>, Self) {
    let messages = ArcShared::new(SpinSyncMutex::new(Vec::<SystemMessage>::new()));
    let sender = Self { messages: messages.clone() };
    (messages, sender)
  }
}

impl ActorRefSender for RecordingSystemMessageSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let system_message = message.downcast_ref::<SystemMessage>().expect("system message payload");
    self.messages.lock().push(system_message.clone());
    Ok(SendOutcome::Delivered)
  }
}

struct StopCountingDriver {
  stop_calls: ArcShared<AtomicUsize>,
}

impl TickDriver for StopCountingDriver {
  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn provision(
    self: Box<Self>,
    _feed: TickFeedHandle,
    _executor: SchedulerTickExecutor,
  ) -> Result<TickDriverProvision, TickDriverError> {
    Ok(TickDriverProvision {
      resolution:    Duration::from_millis(1),
      id:            next_tick_driver_id(),
      kind:          TickDriverKind::Auto,
      stopper:       Box::new(StopCountingStopper { stop_calls: self.stop_calls }),
      auto_metadata: None,
    })
  }
}

#[test]
fn system_state_drop_shuts_down_executor_once() {
  let stop_calls = ArcShared::new(AtomicUsize::new(0));
  let driver = StopCountingDriver { stop_calls: stop_calls.clone() };
  let config = ActorSystemConfig::new(driver);
  let state = SystemState::build_from_owned_config(config).expect("state");
  drop(state);

  assert_eq!(stop_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn system_state_default() {
  let state = SystemState::default();
  assert!(!state.is_terminated());
}

#[test]
fn system_state_allocate_pid() {
  let state = build_state();
  let pid1 = state.allocate_pid();
  let pid2 = state.allocate_pid();
  assert_ne!(pid1.value(), pid2.value());
}

#[test]
fn system_state_monotonic_now() {
  let state = build_state();
  let now1 = state.monotonic_now();
  let now2 = state.monotonic_now();
  assert!(now2 > now1);
}

#[test]
fn system_state_event_stream() {
  let state = build_state();
  let stream = state.event_stream();
  let _ = stream;
}

#[test]
fn system_state_termination_signal() {
  let state = build_state();
  let signal = TerminationSignal::new(state.termination_state());
  assert!(!signal.is_terminated());
}

#[test]
fn system_state_mark_terminated() {
  let state = build_state();
  assert!(!state.is_terminated());
  state.mark_terminated();
  assert!(state.is_terminated());
}

#[test]
fn system_state_register_and_remove_cell() {
  let state = build_shared_state();
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
  let mut state = build_state();
  let pid = state.allocate_pid();
  let path = ActorPath::root().child("reserved").with_uid(ActorUid::new(777));

  state.actor_path_registry_mut().register(pid, &path);

  state.remove_cell(&pid);

  let now = state.monotonic_now().as_secs();
  let result = state.actor_path_registry_mut().reserve_uid(&path, ActorUid::new(888), now, None);
  assert!(matches!(result, Err(PathResolutionError::UidReserved { .. })));
}

#[test]
fn system_state_registers_canonical_uri_with_config() {
  let remoting = RemotingConfig::default().with_canonical_host("localhost").with_canonical_port(2552);
  let config = base_config().with_system_name("fraktor-system").with_remoting_config(remoting);
  let state = SystemStateShared::new(SystemState::build_from_owned_config(config).expect("state"));

  let props = Props::from_fn(|| RestartProbeActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props).expect("worker");
  state.register_cell(child);

  let canonical =
    state.with_actor_path_registry(|registry| registry.canonical_uri(&child_pid).expect("canonical uri").to_string());
  assert!(canonical.starts_with("fraktor.tcp://fraktor-system@localhost:2552"));
  assert!(canonical.ends_with("/user/worker"));
}

#[test]
fn system_state_prefers_advertise_authority_for_canonical_path() {
  let remoting = RemotingConfig::default().with_canonical_host("public.example.com").with_canonical_port(4100);
  let config = base_config().with_system_name("fraktor-system").with_remoting_config(remoting);
  let state = SystemStateShared::new(SystemState::build_from_owned_config(config).expect("state"));

  let props = Props::from_fn(|| RestartProbeActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props).expect("worker");
  state.register_cell(child);

  let canonical = state.canonical_actor_path(&child_pid).expect("canonical path");
  assert_eq!(state.canonical_authority_components(), Some(("public.example.com".to_string(), Some(4100))));
  assert_eq!(state.canonical_authority_endpoint(), Some("public.example.com:4100".to_string()));
  assert_eq!(canonical.parts().scheme(), ActorPathScheme::FraktorTcp);
  assert_eq!(canonical.parts().authority_endpoint(), Some("public.example.com:4100".to_string()));
  assert!(canonical.to_canonical_uri().contains("public.example.com:4100"));
}

#[test]
fn system_state_canonical_authority_endpoint_matches_complete_remoting_config() {
  let remoting = RemotingConfig::default().with_canonical_host("public.example.com").with_canonical_port(4100);
  let config = base_config().with_system_name("fraktor-system").with_remoting_config(remoting);
  let state = SystemState::build_from_owned_config(config).expect("state");

  assert_eq!(state.canonical_authority_components(), Some(("public.example.com".to_string(), Some(4100))));
  assert_eq!(state.canonical_authority_endpoint(), Some("public.example.com:4100".to_string()));
  assert!(!state.has_partial_canonical_authority());
}

#[test]
fn system_state_refuses_canonical_without_port() {
  let remoting = RemotingConfig::default().with_canonical_host("missing-port.example");
  let config = base_config().with_system_name("fraktor-system").with_remoting_config(remoting);
  let state = SystemStateShared::new(SystemState::build_from_owned_config(config).expect("state"));

  let props = Props::from_fn(|| RestartProbeActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props).expect("worker");
  state.register_cell(child);

  assert!(state.canonical_actor_path(&child_pid).is_none());
  assert!(state.with_actor_path_registry(|registry| registry.get(&child_pid).is_none()));
  let local = state.actor_path(&child_pid).expect("local path");
  assert_eq!(local.to_relative_string(), "/user/worker");
  assert!(state.canonical_authority_components().is_none());
}

#[test]
fn system_state_remoting_config_is_none_when_disabled() {
  let config = base_config().with_system_name("fraktor-system");
  let state = SystemState::build_from_owned_config(config).expect("state");
  assert!(state.remoting_config().is_none());
}

#[test]
fn system_state_remoting_config_matches_config_when_enabled() {
  let remoting = RemotingConfig::default()
    .with_canonical_host("example.com")
    .with_canonical_port(2552)
    .with_quarantine_duration(Duration::from_secs(10));
  let config = base_config().with_system_name("fraktor-system").with_remoting_config(remoting.clone());
  let state = SystemState::build_from_owned_config(config).expect("state");

  assert_eq!(state.remoting_config(), Some(remoting));
}

#[test]
fn system_state_remoting_config_retains_partial_authority() {
  let remoting = RemotingConfig::default()
    .with_canonical_host("missing-port.example")
    .with_quarantine_duration(Duration::from_secs(10));
  let config = base_config().with_system_name("fraktor-system").with_remoting_config(remoting.clone());
  let state = SystemStateShared::new(SystemState::build_from_owned_config(config).expect("state"));

  assert_eq!(state.remoting_config(), Some(remoting));
  assert!(state.canonical_authority_components().is_none());
  assert!(state.has_partial_canonical_authority());
}

#[test]
fn system_state_honors_default_guardian_config() {
  let config = base_config().with_system_name("sys-guardian").with_default_guardian(PathGuardianKind::System);
  let state = SystemStateShared::new(SystemState::build_from_owned_config(config).expect("state"));

  let props = Props::from_fn(|| RestartProbeActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "logger".to_string(), &props).expect("logger");
  state.register_cell(child);

  let canonical =
    state.with_actor_path_registry(|registry| registry.canonical_uri(&child_pid).expect("canonical uri").to_string());
  assert!(canonical.contains("/system/logger"), "canonical: {}", canonical);
}

#[test]
fn system_state_assign_name_with_hint() {
  let mut state = build_state();
  let pid = state.allocate_pid();

  let result = state.assign_name(None, Some("test-actor"), pid);
  assert!(result.is_ok());
  let name = result.unwrap();
  assert_eq!(name, "test-actor");
}

#[test]
fn system_state_assign_name_without_hint() {
  let mut state = build_state();
  let pid = state.allocate_pid();

  let result = state.assign_name(None, None, pid);
  assert!(result.is_ok());
  let name = result.unwrap();

  assert!(!name.is_empty());
}

#[test]
fn system_state_release_name() {
  let mut state = build_state();
  let pid = state.allocate_pid();

  let _name = state.assign_name(None, Some("test-actor"), pid).unwrap();
  state.release_name(None, "test-actor");
}

#[test]
fn system_state_user_guardian_pid() {
  let state = build_state();
  assert!(state.user_guardian_pid().is_none());
}

#[test]
fn system_state_child_pids() {
  let state = build_state();
  let parent_pid = state.allocate_pid();

  let children = state.child_pids(parent_pid);
  assert_eq!(children.len(), 0);
}

#[test]
fn system_state_deadletters() {
  let state = build_state();
  let dead_letters = state.dead_letters();
  assert_eq!(dead_letters.len(), 0);
}

#[test]
fn system_state_register_ask_future() {
  use crate::support::futures::{ActorFuture, ActorFutureShared};

  let mut state = build_state();
  let future = ActorFutureShared::new(ActorFuture::new());
  state.register_ask_future(future.clone());

  let ready = state.drain_ready_ask_futures();
  assert_eq!(ready.len(), 0);
}

#[test]
fn system_state_publish_event() {
  use alloc::string::String;
  use core::time::Duration;

  use crate::event::{
    logging::{LogEvent, LogLevel},
    stream::EventStreamEvent,
  };

  let state = build_state();
  let log_event = LogEvent::new(LogLevel::Info, String::from("test"), Duration::from_millis(1), None, None);
  let event = EventStreamEvent::Log(log_event);

  state.publish_event(&event);
}

#[test]
fn system_state_emit_log() {
  use alloc::string::String;

  let state = build_state();
  let events_shared: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(LogRecorder::new(events_shared.clone()));
  let _subscription = state.event_stream().subscribe(&subscriber);
  let pid = state.allocate_pid();

  state.emit_log(LogLevel::Info, String::from("test message"), Some(pid), None);
  state.emit_log(LogLevel::Error, String::from("error message"), None, None);
  state.emit_log(LogLevel::Warn, String::from("named logger message"), Some(pid), Some(String::from("my_logger")));

  let events_snapshot = events_shared.lock().clone();
  let named_log = events_snapshot.iter().rev().find_map(|event| match event {
    | EventStreamEvent::Log(log) if log.message() == "named logger message" => Some(log.clone()),
    | _ => None,
  });

  let named_log = named_log.expect("named logger log event should be published");
  assert_eq!(named_log.logger_name(), Some("my_logger"));
}

#[test]
fn system_state_clear_guardian() {
  let state = build_state();
  let pid = state.allocate_pid();

  let kind = state.guardian_kind_by_pid(pid);
  assert!(kind.is_none());
}

#[test]
fn system_state_user_guardian() {
  let state = build_state();
  assert!(state.user_guardian().is_none());
}

#[test]
fn system_state_register_extra_top_level_success() {
  let mut state = build_state();
  let actor = ActorRef::null();
  assert!(state.register_extra_top_level("metrics", actor.clone()).is_ok());
  assert!(state.extra_top_level("metrics").is_some());
}

#[test]
fn system_state_register_extra_top_level_errors() {
  let mut state = build_state();
  let actor = ActorRef::null();
  let reserved = state.register_extra_top_level("user", actor.clone());
  assert!(matches!(reserved, Err(RegisterExtraTopLevelError::ReservedName(_))));
  state.mark_root_started();
  let started = state.register_extra_top_level("custom", actor);
  assert!(matches!(started, Err(RegisterExtraTopLevelError::AlreadyStarted)));
}

#[test]
fn system_state_temp_actor_round_trip() {
  let mut state = build_state();
  let actor = ActorRef::null();
  let name = state.register_temp_actor(actor.clone());
  assert!(state.temp_actor(&name).is_some());
  state.unregister_temp_actor(&name);
  assert!(state.temp_actor(&name).is_none());
}

#[test]
fn send_system_message_delivers_watch_to_registered_temp_actor_pid() {
  // Given: /temp registry に登録された ActorRef
  let state = build_shared_state();
  let target_pid = state.allocate_pid();
  let watcher_pid = state.allocate_pid();
  let (messages, sender) = RecordingSystemMessageSender::new();
  let target_ref = ActorRef::new_with_builtin_lock(target_pid, sender);
  let _name = state.register_temp_actor(target_ref);

  // When: temp actor pid 宛てに Watch system message を送る
  state.send_system_message(target_pid, SystemMessage::Watch(watcher_pid)).expect("watch delivery");

  // Then: missing actor fallback ではなく、temp actor の sender へ配送される
  assert_eq!(*messages.lock(), vec![SystemMessage::Watch(watcher_pid)]);
}

#[test]
fn send_system_message_delivers_unwatch_to_registered_temp_actor_pid() {
  // Given: /temp registry に登録された ActorRef
  let state = build_shared_state();
  let target_pid = state.allocate_pid();
  let watcher_pid = state.allocate_pid();
  let (messages, sender) = RecordingSystemMessageSender::new();
  let target_ref = ActorRef::new_with_builtin_lock(target_pid, sender);
  let _name = state.register_temp_actor(target_ref);

  // When: temp actor pid 宛てに Unwatch system message を送る
  state.send_system_message(target_pid, SystemMessage::Unwatch(watcher_pid)).expect("unwatch delivery");

  // Then: StageActor::unwatch からの system message も temp actor へ配送できる
  assert_eq!(*messages.lock(), vec![SystemMessage::Unwatch(watcher_pid)]);
}

#[test]
fn system_state_remote_authority_events() {
  let mut state = build_state();
  let stream = state.event_stream();
  let events_shared = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RemoteEventRecorder::new(events_shared.clone()));
  let _subscription = stream.subscribe(&subscriber);

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
fn watch_on_missing_guardian_sends_death_watch_notification_to_watcher() {
  let state = build_shared_state_with_noop_dispatcher();
  let watcher_pid = state.allocate_pid();
  let target_pid = state.allocate_pid();

  let props = Props::from_fn(|| RestartProbeActor).with_dispatcher_id("noop");
  let watcher_cell =
    ActorCell::create(state.clone(), watcher_pid, None, "watcher".to_string(), &props).expect("watcher cell");
  state.register_cell(watcher_cell);

  state.send_system_message(target_pid, SystemMessage::Watch(watcher_pid)).expect("watch send ok");

  let mailbox_snapshot = state.cell(&watcher_pid).expect("watcher cell").mailbox();
  assert_eq!(mailbox_snapshot.system_len(), 1);
  let dequeued = mailbox_snapshot.dequeue_system().expect("dequeue system");
  match dequeued {
    | SystemMessage::DeathWatchNotification(pid) => assert_eq!(pid, target_pid),
    | other => panic!("unexpected system message: {:?}", other),
  }
}

#[test]
fn remote_watch_hook_consumes_watch_skips_fallback() {
  let state = build_shared_state_with_noop_dispatcher();
  let watcher_pid = state.allocate_pid();
  let target_pid = state.allocate_pid();

  let props = Props::from_fn(|| RestartProbeActor).with_dispatcher_id("noop");
  let watcher_cell =
    ActorCell::create(state.clone(), watcher_pid, None, "watcher".to_string(), &props).expect("watcher cell");
  state.register_cell(watcher_cell);

  let calls = ArcShared::new(SpinSyncMutex::new(RemoteWatchHookCalls::default()));
  state.register_remote_watch_hook(Box::new(RecordingRemoteWatchHook::new(calls.clone(), true, false)));

  state.send_system_message(target_pid, SystemMessage::Watch(watcher_pid)).expect("watch send ok");

  let mailbox_snapshot = state.cell(&watcher_pid).expect("watcher cell").mailbox();
  assert_eq!(mailbox_snapshot.system_len(), 0);

  let calls = calls.lock();
  assert_eq!(calls.watch_calls, 1);
  assert_eq!(calls.last_watch, Some((target_pid, watcher_pid)));
}

#[test]
fn remote_watch_hook_non_consuming_watch_runs_fallback() {
  let state = build_shared_state_with_noop_dispatcher();
  let watcher_pid = state.allocate_pid();
  let target_pid = state.allocate_pid();

  let props = Props::from_fn(|| RestartProbeActor).with_dispatcher_id("noop");
  let watcher_cell =
    ActorCell::create(state.clone(), watcher_pid, None, "watcher".to_string(), &props).expect("watcher cell");
  state.register_cell(watcher_cell);

  let calls = ArcShared::new(SpinSyncMutex::new(RemoteWatchHookCalls::default()));
  state.register_remote_watch_hook(Box::new(RecordingRemoteWatchHook::new(calls.clone(), false, false)));

  state.send_system_message(target_pid, SystemMessage::Watch(watcher_pid)).expect("watch send ok");

  let mailbox_snapshot = state.cell(&watcher_pid).expect("watcher cell").mailbox();
  assert_eq!(mailbox_snapshot.system_len(), 1);
  let dequeued = mailbox_snapshot.dequeue_system().expect("dequeue system");
  match dequeued {
    | SystemMessage::DeathWatchNotification(pid) => assert_eq!(pid, target_pid),
    | other => panic!("unexpected system message: {:?}", other),
  }

  let calls = calls.lock();
  assert_eq!(calls.watch_calls, 1);
  assert_eq!(calls.last_watch, Some((target_pid, watcher_pid)));
}

#[test]
fn remote_watch_hook_consumes_unwatch_is_invoked() {
  let state = build_shared_state();
  let watcher_pid = state.allocate_pid();
  let target_pid = state.allocate_pid();

  let calls = ArcShared::new(SpinSyncMutex::new(RemoteWatchHookCalls::default()));
  state.register_remote_watch_hook(Box::new(RecordingRemoteWatchHook::new(calls.clone(), false, true)));

  state.send_system_message(target_pid, SystemMessage::Unwatch(watcher_pid)).expect("unwatch send ok");

  let calls = calls.lock();
  assert_eq!(calls.unwatch_calls, 1);
  assert_eq!(calls.last_unwatch, Some((target_pid, watcher_pid)));
}

#[test]
fn remote_watch_hook_replaces_previous_registration() {
  let state = build_shared_state_with_noop_dispatcher();
  let watcher_pid = state.allocate_pid();
  let target_pid = state.allocate_pid();

  let props = Props::from_fn(|| RestartProbeActor).with_dispatcher_id("noop");
  let watcher_cell =
    ActorCell::create(state.clone(), watcher_pid, None, "watcher".to_string(), &props).expect("watcher cell");
  state.register_cell(watcher_cell);

  let calls1 = ArcShared::new(SpinSyncMutex::new(RemoteWatchHookCalls::default()));
  state.register_remote_watch_hook(Box::new(RecordingRemoteWatchHook::new(calls1.clone(), false, false)));

  let calls2 = ArcShared::new(SpinSyncMutex::new(RemoteWatchHookCalls::default()));
  state.register_remote_watch_hook(Box::new(RecordingRemoteWatchHook::new(calls2.clone(), true, false)));

  state.send_system_message(target_pid, SystemMessage::Watch(watcher_pid)).expect("watch send ok");

  let mailbox_snapshot = state.cell(&watcher_pid).expect("watcher cell").mailbox();
  assert_eq!(mailbox_snapshot.system_len(), 0);

  assert_eq!(calls1.lock().watch_calls, 0);
  assert_eq!(calls2.lock().watch_calls, 1);
}

#[test]
fn termination_signal_completes_after_root_marked_terminated() {
  let state = build_shared_state();
  let root_pid = state.allocate_pid();
  let props = Props::from_fn(|| RestartProbeActor);
  let root_cell =
    ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root guardian cell");
  state.set_root_guardian(&root_cell);

  assert!(!state.termination_signal().is_terminated());
  assert_eq!(state.guardian_kind_by_pid(root_pid), Some(GuardianKind::Root));
  state.mark_guardian_stopped(GuardianKind::Root);
  state.mark_terminated();

  assert!(state.termination_signal().is_terminated());
}

#[test]
fn system_state_logs_failure_with_pid_origin() {
  use core::time::Duration;

  let state = build_shared_state();
  let events_shared: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(LogRecorder::new(events_shared.clone()));
  let _subscription = state.event_stream().subscribe(&subscriber);

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

#[derive(Default)]
struct RemoteWatchHookCalls {
  watch_calls:   usize,
  unwatch_calls: usize,
  last_watch:    Option<(Pid, Pid)>,
  last_unwatch:  Option<(Pid, Pid)>,
}

struct RecordingRemoteWatchHook {
  calls:           ArcShared<SpinSyncMutex<RemoteWatchHookCalls>>,
  consume_watch:   bool,
  consume_unwatch: bool,
}

impl RecordingRemoteWatchHook {
  fn new(calls: ArcShared<SpinSyncMutex<RemoteWatchHookCalls>>, consume_watch: bool, consume_unwatch: bool) -> Self {
    Self { calls, consume_watch, consume_unwatch }
  }
}

impl crate::system::remote::RemoteWatchHook for RecordingRemoteWatchHook {
  fn handle_watch(&mut self, target: Pid, watcher: Pid) -> bool {
    let mut calls = self.calls.lock();
    calls.watch_calls += 1;
    calls.last_watch = Some((target, watcher));
    self.consume_watch
  }

  fn handle_unwatch(&mut self, target: Pid, watcher: Pid) -> bool {
    let mut calls = self.calls.lock();
    calls.unwatch_calls += 1;
    calls.last_unwatch = Some((target, watcher));
    self.consume_unwatch
  }
}

struct RemoteEventRecorder {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl RemoteEventRecorder {
  fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl Default for RemoteEventRecorder {
  fn default() -> Self {
    Self::new(ArcShared::new(SpinSyncMutex::new(Vec::new())))
  }
}

impl EventStreamSubscriber for RemoteEventRecorder {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

/// Noop executor used to verify that spawn paths never block on dispatcher
/// progress. `execute` discards the submitted closure so the mailbox never
/// drains.
struct NoopExecutor;

impl Executor for NoopExecutor {
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn noop_dispatcher_configurator() -> ArcShared<Box<dyn MessageDispatcherFactory>> {
  use crate::dispatch::dispatcher::ExecutorShared;
  let settings = DispatcherConfig::with_defaults("noop");
  let executor = ExecutorShared::new(Box::new(NoopExecutor), TrampolineState::new());
  let configurator: Box<dyn MessageDispatcherFactory> = Box::new(DefaultDispatcherFactory::new(&settings, executor));
  ArcShared::new(configurator)
}

struct LogRecorder {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl LogRecorder {
  fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl Default for LogRecorder {
  fn default() -> Self {
    Self::new(ArcShared::new(SpinSyncMutex::new(Vec::new())))
  }
}

impl EventStreamSubscriber for LogRecorder {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

impl Actor for RestartProbeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}
