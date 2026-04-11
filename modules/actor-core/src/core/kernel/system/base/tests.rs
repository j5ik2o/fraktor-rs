use alloc::{boxed::Box, format, vec, vec::Vec};
use core::{
  pin::Pin,
  sync::atomic::{AtomicBool, AtomicUsize, Ordering},
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  time::Duration,
};

use fraktor_utils_core_rs::core::{
  collections::queue::capabilities::{QueueCapabilityRegistry, QueueCapabilitySet},
  sync::{ArcShared, SharedAccess, SharedLock, SpinSyncMutex},
  timing::delay::{DelayFuture, DelayProvider},
};

use super::ActorSystem;
use crate::core::{
  kernel::{
    actor::{
      Actor, ActorCell, ActorContext, Pid,
      actor_path::{ActorPath, ActorPathParts, ActorPathScheme},
      actor_ref::ActorRef,
      actor_ref_provider::{ActorRefProvider, ActorRefProviderShared, ActorRefResolveError},
      error::ActorError,
      lifecycle::LifecycleStage,
      messaging::{AnyMessageView, system_message::SystemMessage},
      props::{MailboxConfig, MailboxRequirement, Props},
      scheduler::{
        SchedulerConfig,
        task_run::{TaskRunError, TaskRunPriority},
        tick_driver::{
          AutoDriverMetadata, AutoProfileKind, ManualTestDriver, SchedulerTickExecutor, TickDriver, TickDriverConfig,
          TickDriverControl, TickDriverError, TickDriverHandle, TickDriverId, TickDriverKind, TickExecutorPump,
          TickFeedHandle,
        },
      },
      setup::ActorSystemConfig,
      spawn::SpawnError,
    },
    dispatch::dispatcher::{
      DefaultDispatcherConfigurator, DispatcherSettings, ExecuteError, Executor, ExecutorShared,
      MessageDispatcherConfigurator,
    },
    event::stream::{EventStreamEvent, EventStreamSubscriber, tests::subscriber_handle},
    system::{
      TerminationSignal,
      base::LogLevel,
      lock_provider::{ActorLockProvider, BuiltinSpinLockProvider},
      remote::RemotingConfig,
      state::{SystemStateShared, system_state::SystemState},
    },
  },
  typed::receptionist::SYSTEM_RECEPTIONIST_TOP_LEVEL,
};

struct TestActor;

impl Actor for TestActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct SpawnRecorderActor {
  log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl SpawnRecorderActor {
  fn new(log: ArcShared<SpinSyncMutex<Vec<&'static str>>>) -> Self {
    Self { log }
  }
}

impl Actor for SpawnRecorderActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("pre_start");
    Ok(())
  }

  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.log.lock().push("receive");
    Ok(())
  }
}

struct FailingStartActor;

impl Actor for FailingStartActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    Err(ActorError::recoverable("boom"))
  }
}

struct LifecycleEventWatcher {
  stages: ArcShared<SpinSyncMutex<Vec<LifecycleStage>>>,
}

impl LifecycleEventWatcher {
  fn new(stages: ArcShared<SpinSyncMutex<Vec<LifecycleStage>>>) -> Self {
    Self { stages }
  }
}

impl EventStreamSubscriber for LifecycleEventWatcher {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Lifecycle(lifecycle) = event {
      self.stages.lock().push(lifecycle.stage());
    }
  }
}

#[test]
fn spawn_child_fails_before_root_started() {
  let system = ActorSystem::new_empty();
  let props = Props::from_fn(|| TestActor);
  let err = system.spawn_child(Pid::new(999, 0), &props).unwrap_err();
  assert!(matches!(err, SpawnError::InvalidProps(_)));
}

#[test]
fn resolve_actor_ref_fails_before_root_started() {
  let system = ActorSystem::new_empty();
  let path = ActorPath::root();
  let err = system.resolve_actor_ref(path).unwrap_err();
  assert!(matches!(err, ActorRefResolveError::ProviderMissing | ActorRefResolveError::InvalidAuthority));
}

/// Noop executor used to verify that spawn paths never block on dispatcher
/// progress. `execute` discards the submitted closure so the mailbox never
/// drains.
struct NoopExecutor;

impl Executor for NoopExecutor {
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn noop_dispatcher_configurator() -> ArcShared<Box<dyn MessageDispatcherConfigurator>> {
  let lock_provider: ArcShared<dyn ActorLockProvider> = ArcShared::new(BuiltinSpinLockProvider::new());
  let settings = DispatcherSettings::with_defaults("noop");
  let executor = ExecutorShared::new_with_builtin_lock(NoopExecutor);
  let configurator: Box<dyn MessageDispatcherConfigurator> =
    Box::new(DefaultDispatcherConfigurator::new(&settings, executor, &lock_provider));
  ArcShared::new(configurator)
}

struct NoopControl;

impl TickDriverControl for NoopControl {
  fn shutdown(&self) {}
}

struct StaticTickDriver {
  id:         TickDriverId,
  kind:       TickDriverKind,
  resolution: Duration,
}

impl StaticTickDriver {
  const fn new(id: TickDriverId, kind: TickDriverKind, resolution: Duration) -> Self {
    Self { id, kind, resolution }
  }
}

impl TickDriver for StaticTickDriver {
  fn id(&self) -> TickDriverId {
    self.id
  }

  fn kind(&self) -> TickDriverKind {
    self.kind
  }

  fn resolution(&self) -> Duration {
    self.resolution
  }

  fn start(&mut self, _feed: TickFeedHandle) -> Result<TickDriverHandle, TickDriverError> {
    let control: Box<dyn TickDriverControl> = Box::new(NoopControl);
    let control = SharedLock::new_with_driver::<SpinSyncMutex<_>>(control);
    Ok(TickDriverHandle::new(self.id, self.kind, self.resolution, control))
  }
}

struct ShutdownRecordingControl {
  shutdown_calls: ArcShared<AtomicUsize>,
  did_shutdown:   AtomicBool,
}

impl ShutdownRecordingControl {
  fn new(shutdown_calls: ArcShared<AtomicUsize>) -> Self {
    Self { shutdown_calls, did_shutdown: AtomicBool::new(false) }
  }
}

impl TickDriverControl for ShutdownRecordingControl {
  fn shutdown(&self) {
    if !self.did_shutdown.swap(true, Ordering::SeqCst) {
      self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
    }
  }
}

struct ShutdownRecordingPump {
  shutdown_calls: ArcShared<AtomicUsize>,
  profile:        Option<AutoProfileKind>,
}

impl ShutdownRecordingPump {
  fn new(shutdown_calls: ArcShared<AtomicUsize>, profile: Option<AutoProfileKind>) -> Self {
    Self { shutdown_calls, profile }
  }
}

impl TickExecutorPump for ShutdownRecordingPump {
  fn spawn(&mut self, _executor: SchedulerTickExecutor) -> Result<Box<dyn TickDriverControl>, TickDriverError> {
    Ok(Box::new(ShutdownRecordingControl::new(self.shutdown_calls.clone())))
  }

  fn auto_metadata(&self, driver_id: TickDriverId, resolution: Duration) -> Option<AutoDriverMetadata> {
    self.profile.map(|profile| AutoDriverMetadata { profile, driver_id, resolution })
  }
}

#[test]
fn actor_system_new_empty() {
  let system = ActorSystem::new_empty();
  assert!(!system.state().is_terminated());
}

#[test]
fn actor_system_new_empty_provides_manual_tick_driver_and_runner_api() {
  let system = ActorSystem::new_empty();
  let snapshot = system.tick_driver_snapshot().expect("tick driver snapshot");
  assert_eq!(snapshot.kind, TickDriverKind::ManualTest);
  assert!(system.scheduler().with_read(|s| s.config().runner_api_enabled()));
}

#[test]
fn actor_system_drop_shuts_down_executor_once() {
  let executor_calls = ArcShared::new(AtomicUsize::new(0));
  let tick_driver = TickDriverConfig::runtime(
    Box::new(StaticTickDriver::new(TickDriverId::new(1), TickDriverKind::Auto, Duration::from_millis(1))),
    Box::new(ShutdownRecordingPump::new(executor_calls.clone(), None)),
  );
  let config = ActorSystemConfig::default().with_tick_driver(tick_driver);

  let state = SystemState::build_from_config(&config).expect("state");
  let system = ActorSystem::from_state(SystemStateShared::new(state));
  drop(system);

  assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn actor_system_new_with_config_and_allows_extra_top_level_registration_in_configure() {
  let props = Props::from_fn(|| TestActor);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);

  let system = ActorSystem::new_with_config_and(&props, &config, |system| {
    assert!(!system.state().has_root_started());
    let actor = ActorRef::null();
    system
      .extended()
      .register_extra_top_level("metrics", actor)
      .map_err(|error| SpawnError::SystemBuildError(format!("{error:?}")))?;
    Ok(())
  })
  .expect("system should build");

  assert!(system.state().has_root_started());
  assert!(system.state().extra_top_level("metrics").is_some());

  let late = system.extended().register_extra_top_level("late", ActorRef::null());
  assert!(matches!(late, Err(crate::core::kernel::system::RegisterExtraTopLevelError::AlreadyStarted)));
}

#[test]
fn actor_system_registers_system_receptionist_during_bootstrap() {
  let props = Props::from_fn(|| TestActor);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);

  let system = ActorSystem::new_with_config_and(&props, &config, |_| Ok(())).expect("system should build");

  assert!(system.state().extra_top_level(SYSTEM_RECEPTIONIST_TOP_LEVEL).is_some());
}

#[test]
fn bootstrap_rolls_back_receptionist_when_extra_top_level_registration_fails() {
  let props = Props::from_fn(|| TestActor);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  let state = SystemStateShared::new(SystemState::build_from_config(&config).expect("state"));
  state.register_extra_top_level(SYSTEM_RECEPTIONIST_TOP_LEVEL, ActorRef::null()).expect("pre-register receptionist");
  let system = ActorSystem::from_state(state);

  let result = system.bootstrap(&props, |_| Ok(()));

  match result {
    | Err(SpawnError::SystemBuildError(message)) => {
      assert!(message.contains("system receptionist registration failed"));
      assert!(message.contains("DuplicateName"));
    },
    | other => panic!("unexpected bootstrap result: {other:?}"),
  }

  let system_guardian_pid = system.state().system_guardian_pid().expect("system guardian pid");
  assert!(system.children(system_guardian_pid).is_empty());
  assert!(system.state().extra_top_level(SYSTEM_RECEPTIONIST_TOP_LEVEL).is_some());
}

#[test]
fn actor_system_new_with_config_and_fails_without_tick_driver_config() {
  let props = Props::from_fn(|| TestActor);
  let config = ActorSystemConfig::default();
  match ActorSystem::new_with_config_and(&props, &config, |_| Ok(())) {
    | Ok(_) => panic!("system should not build without tick driver config"),
    | Err(SpawnError::SystemBuildError(message)) => assert!(message.contains("tick driver configuration is required")),
    | Err(other) => panic!("unexpected error: {other:?}"),
  };
}

#[test]
fn actor_system_from_state() {
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  let state = SystemState::build_from_config(&config).expect("state");
  let system = ActorSystem::from_state(SystemStateShared::new(state));
  assert!(!system.state().is_terminated());
}

#[test]
fn actor_system_clone() {
  let system1 = ActorSystem::new_empty();
  let system2 = system1.clone();
  assert!(!system1.state().is_terminated());
  assert!(!system2.state().is_terminated());
}

#[test]
fn actor_system_allocate_pid() {
  let system = ActorSystem::new_empty();
  let pid1 = system.allocate_pid();
  let pid2 = system.allocate_pid();
  assert_ne!(pid1.value(), pid2.value());
}

#[test]
fn actor_system_state() {
  let system = ActorSystem::new_empty();
  let state = system.state();
  assert!(!state.is_terminated());
}

#[test]
fn actor_system_event_stream() {
  let system = ActorSystem::new_empty();
  let stream = system.event_stream();
  let _ = stream;
}

#[test]
fn actor_system_deadletters() {
  let system = ActorSystem::new_empty();
  let deadletters = system.dead_letters();
  assert_eq!(deadletters.len(), 0);
}

#[test]
fn actor_system_emit_log() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  system.emit_log(LogLevel::Info, "test message", Some(pid), None);
}

#[test]
fn actor_system_when_terminated() {
  let system = ActorSystem::new_empty();
  let signal = system.when_terminated();
  assert!(!signal.is_terminated());
}

#[test]
fn actor_system_reports_tick_driver_snapshot() {
  let driver_id = TickDriverId::new(99);
  let resolution = Duration::from_millis(5);
  let tick_driver = TickDriverConfig::runtime(
    Box::new(StaticTickDriver::new(driver_id, TickDriverKind::Auto, resolution)),
    Box::new(ShutdownRecordingPump::new(ArcShared::new(AtomicUsize::new(0)), Some(AutoProfileKind::Tokio))),
  );
  let config = ActorSystemConfig::default().with_tick_driver(tick_driver);
  let state = SystemState::build_from_config(&config).expect("state");
  let system = ActorSystem::from_state(SystemStateShared::new(state));

  let snapshot = system.tick_driver_snapshot().expect("tick driver snapshot");
  assert_eq!(snapshot.metadata.driver_id, driver_id);
  assert_eq!(snapshot.kind, TickDriverKind::Auto);
  assert_eq!(snapshot.resolution, resolution);
  assert_eq!(snapshot.auto.as_ref().map(|meta| meta.profile), Some(AutoProfileKind::Tokio));
}

#[test]
fn actor_system_actor_ref_for_nonexistent_pid() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  assert!(system.actor_ref(pid).is_none());
}

#[test]
fn actor_system_children_for_nonexistent_parent() {
  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let children = system.children(parent_pid);
  assert_eq!(children.len(), 0);
}

#[test]
fn actor_system_spawn_child_with_invalid_parent() {
  let system = ActorSystem::new_empty();
  let props = Props::from_fn(|| TestActor);
  let invalid_parent = system.allocate_pid();

  let result = system.spawn_child(invalid_parent, &props);
  assert!(result.is_err());
}

#[test]
fn spawn_child_fails_when_deque_requirement_missing() {
  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let parent_name = system.state().assign_name(None, Some("parent"), parent_pid).expect("parent name");
  let parent_cell = ActorCell::create(system.state(), parent_pid, None, parent_name, &Props::from_fn(|| TestActor))
    .expect("create actor cell");
  system.state().register_cell(parent_cell);

  let capabilities = QueueCapabilityRegistry::new(QueueCapabilitySet::defaults().with_deque(false));
  let mailbox =
    MailboxConfig::default().with_capabilities(capabilities).with_requirement(MailboxRequirement::for_stash());
  let props = Props::from_fn(|| TestActor).with_mailbox_config(mailbox);

  let result = system.spawn_child(parent_pid, &props);
  assert!(matches!(result, Err(SpawnError::InvalidProps(_))));
}

#[test]
fn spawn_child_succeeds_when_requirements_met() {
  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let parent_name = system.state().assign_name(None, Some("parent"), parent_pid).expect("parent name");
  let parent_cell = ActorCell::create(system.state(), parent_pid, None, parent_name, &Props::from_fn(|| TestActor))
    .expect("create actor cell");
  system.state().register_cell(parent_cell);

  let mailbox = MailboxConfig::default().with_requirement(MailboxRequirement::for_stash());
  let props = Props::from_fn(|| TestActor).with_mailbox_config(mailbox);

  assert!(system.spawn_child(parent_pid, &props).is_ok());
}

#[test]
fn spawn_child_fails_when_dispatcher_id_not_registered() {
  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let parent_name = system.state().assign_name(None, Some("parent"), parent_pid).expect("parent name");
  let parent_cell = ActorCell::create(system.state(), parent_pid, None, parent_name, &Props::from_fn(|| TestActor))
    .expect("create actor cell");
  system.state().register_cell(parent_cell);

  let props = Props::from_fn(|| TestActor).with_dispatcher_id("custom-dispatcher");
  let result = system.spawn_child(parent_pid, &props);
  assert!(matches!(result, Err(SpawnError::InvalidProps(_))));
}

#[test]
fn spawn_child_resolves_mailbox_id_with_requirements() {
  use fraktor_utils_core_rs::core::collections::queue::capabilities::{QueueCapabilityRegistry, QueueCapabilitySet};

  let registry = QueueCapabilityRegistry::new(QueueCapabilitySet::defaults().with_deque(false));
  let constrained =
    MailboxConfig::default().with_requirement(MailboxRequirement::requires_deque()).with_capabilities(registry);

  let system = ActorSystem::new_empty_with(|config| config.with_mailbox("constrained", constrained));
  let parent_pid = system.allocate_pid();
  let parent_name = system.state().assign_name(None, Some("parent"), parent_pid).expect("parent name");
  let parent_cell = ActorCell::create(system.state(), parent_pid, None, parent_name, &Props::from_fn(|| TestActor))
    .expect("create actor cell");
  system.state().register_cell(parent_cell);

  let props = Props::from_fn(|| TestActor)
    .with_mailbox_id("constrained")
    .with_mailbox_requirement(MailboxRequirement::for_stash());

  let result = system.spawn_child(parent_pid, &props);
  assert!(matches!(result, Err(SpawnError::InvalidProps(_))));
}

#[test]
fn actor_system_spawn_without_guardian() {
  let system = ActorSystem::new_empty();
  let props = Props::from_fn(|| TestActor);

  let result = system.spawn(&props);
  assert!(result.is_err());
}

fn make_test_system() -> ActorSystem {
  make_test_system_with_name("test-system")
}

fn make_test_system_with_name(name: &str) -> ActorSystem {
  let props = Props::from_fn(|| TestActor);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfig::default().with_system_name(name).with_tick_driver(tick_driver);
  ActorSystem::new_with_config(&props, &config).expect("system")
}

#[test]
fn actor_system_actor_of_spawns_under_user_guardian() {
  let system = make_test_system();

  let child = system.actor_of(&Props::from_fn(|| TestActor)).expect("spawn child");
  let path = child.actor_ref().path().expect("child path");

  assert!(path.to_relative_string().starts_with("/user/"));
  assert!(system.state().cell(&child.pid()).is_some());
}

#[test]
fn actor_system_actor_of_named_uses_requested_name() {
  let system = make_test_system();

  let child = system.actor_of_named(&Props::from_fn(|| TestActor), "named-child").expect("spawn child");
  let path = child.actor_ref().path().expect("child path");

  assert!(path.to_relative_string().ends_with("/named-child"));
}

#[test]
fn actor_system_actor_of_named_rejects_duplicate_name() {
  let system = make_test_system();

  let first = system.actor_of_named(&Props::from_fn(|| TestActor), "dup-name");
  assert!(first.is_ok());

  let second = system.actor_of_named(&Props::from_fn(|| TestActor), "dup-name");
  assert!(matches!(second, Err(SpawnError::NameConflict(_))));
}

#[test]
fn actor_system_stop_stops_target_actor() {
  let system = make_test_system();

  let child = system.actor_of_named(&Props::from_fn(|| TestActor), "stop-target").expect("spawn child");
  let actor = child.actor_ref().clone();

  system.stop(&actor).expect("stop");
  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));

  assert!(system.state().cell(&actor.pid()).is_none());
}

#[test]
fn extended_actor_system_exposes_actor_ref_factory_surface() {
  let system = make_test_system();
  let extended = system.extended();

  let child = extended.actor_of_named(&Props::from_fn(|| TestActor), "extended-child").expect("spawn child");
  let actor = child.actor_ref().clone();

  assert!(child.actor_ref().path().expect("path").to_relative_string().ends_with("/extended-child"));
  extended.stop(&actor).expect("stop");
  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));
  assert!(system.state().cell(&actor.pid()).is_none());
}

#[test]
fn extended_actor_system_exposes_actor_selection_surface() {
  let system = make_test_system_with_name("extended-selection-system");
  let extended = system.extended();

  let child = extended.actor_of_named(&Props::from_fn(|| TestActor), "extended-selection-child").expect("spawn child");
  let path = child.actor_ref().path().expect("path");

  let by_string =
    extended.actor_selection(&path.to_relative_string()).to_serialization_format().expect("serialize by string");
  let by_path = extended.actor_selection_from_path(&path).to_serialization_format().expect("serialize by path");

  assert!(by_string.ends_with("/extended-selection-child"));
  assert!(by_path.ends_with("/extended-selection-child"));
}

#[test]
fn actor_system_drain_ready_ask_futures() {
  let system = ActorSystem::new_empty();
  let futures = system.drain_ready_ask_futures();
  assert_eq!(futures.len(), 0);
}

#[test]
fn actor_system_terminate_without_guardian() {
  let system = ActorSystem::new_empty();
  let result = system.terminate();
  assert!(result.is_ok());
  assert!(system.state().is_terminated());
}

#[test]
fn actor_system_terminate_when_already_terminated() {
  let system = ActorSystem::new_empty();
  system.state().mark_terminated();
  let result = system.terminate();
  assert!(result.is_ok());
}

#[test]
fn spawn_does_not_block_when_dispatcher_never_runs() {
  // Register NoopExecutor as "noop" dispatcher
  let system =
    ActorSystem::new_empty_with(|config| config.with_dispatcher_configurator("noop", noop_dispatcher_configurator()));
  let log: ArcShared<SpinSyncMutex<Vec<&'static str>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));

  let props = Props::from_fn({
    let log = log.clone();
    move || SpawnRecorderActor::new(log.clone())
  })
  .with_dispatcher_id("noop");

  let child = system.spawn_with_parent(None, &props).expect("spawn succeeds");
  assert!(log.lock().is_empty());
  assert!(system.state().cell(&child.pid()).is_some());
}

#[test]
fn spawn_child_same_as_parent_inherits_dispatcher_selection_result() {
  let system =
    ActorSystem::new_empty_with(|config| config.with_dispatcher_configurator("noop", noop_dispatcher_configurator()));

  let parent_props = Props::from_fn(|| TestActor).with_dispatcher_id("noop");
  let parent = system.spawn_with_parent(None, &parent_props).expect("parent spawn succeeds");

  let child_props = Props::from_fn(|| TestActor).with_dispatcher_same_as_parent();
  let child = system.spawn_with_parent(Some(parent.pid()), &child_props).expect("child spawn succeeds");

  let parent_cell = system.state().cell(&parent.pid()).expect("parent cell");
  let child_cell = system.state().cell(&child.pid()).expect("child cell");
  assert_eq!(parent_cell.dispatcher_id(), child_cell.dispatcher_id());
}

#[test]
fn spawn_succeeds_even_if_pre_start_fails() {
  let system = ActorSystem::new_empty();
  let props = Props::from_fn(|| FailingStartActor);
  let child = system.spawn_with_parent(None, &props).expect("spawn succeeds despite failure");

  assert!(system.state().cell(&child.pid()).is_none());
}

#[test]
fn create_send_failure_triggers_rollback() {
  let system = ActorSystem::new_empty();
  let props = Props::from_fn(|| TestActor);
  let pid = system.allocate_pid();
  let name = system.state().assign_name(None, props.name(), pid).expect("name assigned");
  let cell = system.build_cell_for_spawn(pid, None, name, &props).expect("セル生成に失敗");
  system.state().register_cell(cell.clone());

  system.state().remove_cell(&pid);
  let result = system.perform_create_handshake(None, pid, &cell);

  match result {
    | Err(SpawnError::InvalidProps(reason)) => {
      assert_eq!(reason, super::CREATE_SEND_FAILED);
    },
    | other => panic!("unexpected handshake result: {:?}", other),
  }

  assert!(system.state().cell(&pid).is_none());
  let retry = system.state().assign_name(None, Some(cell.name()), pid);
  assert!(retry.is_ok());
}

#[test]
fn spawn_returns_child_ref_even_if_dispatcher_is_idle() {
  let system =
    ActorSystem::new_empty_with(|config| config.with_dispatcher_configurator("noop", noop_dispatcher_configurator()));
  let props = Props::from_fn(|| TestActor).with_dispatcher_id("noop");
  let result = system.spawn_with_parent(None, &props);

  assert!(result.is_ok());
}

fn new_noop_waker() -> Waker {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(|data| RawWaker::new(data, &VTABLE), |_| {}, |_| {}, |_| {});

  unsafe fn raw_waker() -> RawWaker {
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  unsafe { Waker::from_raw(raw_waker()) }
}

fn poll_delay(future: &mut DelayFuture) -> Poll<()> {
  let waker = new_noop_waker();
  let mut cx = Context::from_waker(&waker);
  Pin::new(future).poll(&mut cx)
}

#[test]
fn actor_system_scheduler_handles_delays() {
  let props = Props::from_fn(|| TestActor);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let system = ActorSystem::new(&props, tick_driver).expect("system");
  let mut provider = system.delay_provider();
  let mut future = provider.delay(Duration::from_millis(1));
  assert!(matches!(poll_delay(&mut future), Poll::Pending));

  let scheduler = system.scheduler();
  scheduler.with_write(|s| s.run_for_test(1));

  assert!(matches!(poll_delay(&mut future), Poll::Ready(())));
}

#[test]
fn actor_system_terminate_runs_scheduler_tasks() {
  let props = Props::from_fn(|| TestActor);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let system = ActorSystem::new(&props, tick_driver).expect("system");
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  {
    let scheduler = system.scheduler();
    scheduler.with_write(|s| {
      let task = RecordingShutdownTask { log: log.clone() };
      s.register_on_close(task, TaskRunPriority::User).expect("register");
    });
  }

  system.terminate().expect("terminate");

  assert_eq!(log.lock().as_slice(), &["shutdown"]);
}

struct RecordingShutdownTask {
  log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl crate::core::kernel::actor::scheduler::task_run::TaskRunOnClose for RecordingShutdownTask {
  fn run(&mut self) -> Result<(), TaskRunError> {
    self.log.lock().push("shutdown");
    Ok(())
  }
}

fn noop_waker() -> Waker {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(|data| RawWaker::new(data, &VTABLE), |_| {}, |_| {}, |_| {});

  unsafe fn raw_waker() -> RawWaker {
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  unsafe { Waker::from_raw(raw_waker()) }
}

fn poll_delay_future(future: &mut DelayFuture) -> Poll<()> {
  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);
  Pin::new(future).poll(&mut cx)
}

#[test]
fn actor_system_installs_scheduler() {
  let props = Props::from_fn(|| TestActor);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let system = ActorSystem::new(&props, tick_driver).expect("actor system");
  let mut provider = system.delay_provider();
  let mut future = provider.delay(Duration::from_millis(1));
  assert!(matches!(poll_delay_future(&mut future), Poll::Pending));

  system.scheduler().with_write(|s| s.run_for_test(1));

  assert!(matches!(poll_delay_future(&mut future), Poll::Ready(())));
}

#[test]
fn lifecycle_events_cover_restart_transitions() {
  let system = ActorSystem::new_empty();
  let stages: ArcShared<SpinSyncMutex<Vec<LifecycleStage>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(LifecycleEventWatcher::new(stages.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  let props = Props::from_fn(|| TestActor);
  let child = system.spawn_with_parent(None, &props).expect("spawn succeeds");
  let pid = child.pid();

  system.state().send_system_message(pid, SystemMessage::Recreate).expect("recreate enqueued");

  let snapshot = stages.lock().clone();
  assert_eq!(snapshot, vec![LifecycleStage::Started, LifecycleStage::Stopped, LifecycleStage::Restarted]);
}

struct DummyActorRefProvider {
  last_path: ArcShared<SpinSyncMutex<Option<ActorPath>>>,
}

impl DummyActorRefProvider {
  fn new(last_path: ArcShared<SpinSyncMutex<Option<ActorPath>>>) -> Self {
    Self { last_path }
  }
}

impl ActorRefProvider for DummyActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    &[ActorPathScheme::FraktorTcp]
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    *self.last_path.lock() = Some(path.clone());
    Ok(ActorRef::null())
  }

  fn termination_signal(&self) -> TerminationSignal {
    TerminationSignal::already_terminated()
  }
}

#[test]
fn resolve_actor_ref_injects_canonical_authority() {
  let remoting = RemotingConfig::default().with_canonical_host("example.com").with_canonical_port(2552);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfig::default().with_remoting_config(remoting).with_tick_driver(tick_driver);
  let state = SystemState::build_from_config(&config).expect("state");
  let system = ActorSystem::from_state(SystemStateShared::new(state));

  let recorded = ArcShared::new(SpinSyncMutex::new(None));
  let provider = ActorRefProviderShared::new(DummyActorRefProvider::new(recorded.clone()));
  system.extended().register_actor_ref_provider(&provider).expect("register provider");
  system.state().mark_root_started();

  let path = ActorPath::root().child("svc");
  let resolved = system.resolve_actor_ref(path.clone());

  assert!(resolved.is_ok());
  let stored = recorded.lock().clone().expect("path recorded");
  assert_eq!(stored.parts().scheme(), ActorPathScheme::FraktorTcp);
  assert_eq!(stored.parts().authority_endpoint().as_deref(), Some("example.com:2552"));
  assert_eq!(stored.to_relative_string(), path.to_relative_string());
}

#[test]
fn resolve_actor_ref_fails_when_authority_missing() {
  let system = ActorSystem::new_empty();
  let parts = ActorPathParts::local("cellactor").with_scheme(ActorPathScheme::FraktorTcp);
  let path = ActorPath::from_parts(parts).child("svc");

  let result = system.resolve_actor_ref(path);
  assert!(matches!(result, Err(ActorRefResolveError::InvalidAuthority)));
}

#[test]
fn resolve_actor_ref_fails_when_provider_missing() {
  let remoting = RemotingConfig::default().with_canonical_host("example.com").with_canonical_port(2552);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfig::default().with_remoting_config(remoting).with_tick_driver(tick_driver);
  let state = SystemState::build_from_config(&config).expect("state");
  let system = ActorSystem::from_state(SystemStateShared::new(state));
  system.state().mark_root_started();

  let path = ActorPath::root().child("svc");
  let result = system.resolve_actor_ref(path);

  assert!(matches!(result, Err(ActorRefResolveError::ProviderMissing)));
}

#[test]
fn guardian_refs_preserve_canonical_authority() {
  let user_props = Props::from_fn(|| TestActor).with_name("user-guardian");
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let remoting = RemotingConfig::default().with_canonical_host("guardian.example.com").with_canonical_port(4101);
  let config = ActorSystemConfig::default()
    .with_system_name("guardian-compat")
    .with_remoting_config(remoting)
    .with_tick_driver(tick_driver);

  let system = ActorSystem::new_with_config(&user_props, &config).expect("actor system bootstrap");

  let user_pid = system.state().user_guardian_pid().expect("user guardian pid");
  let user_ref = system.user_guardian_ref();
  assert_eq!(user_ref.pid(), user_pid);

  let user_canonical = user_ref.canonical_path().expect("canonical path for user guardian");
  assert_eq!(user_canonical.parts().scheme(), ActorPathScheme::FraktorTcp);
  assert_eq!(user_canonical.parts().authority_endpoint().as_deref(), Some("guardian.example.com:4101"));

  let system_ref = system.system_guardian_ref().expect("system guardian ref");
  let system_canonical = system_ref.canonical_path().expect("canonical path for system guardian");
  assert_eq!(system_canonical.parts().scheme(), ActorPathScheme::FraktorTcp);
  assert_eq!(system_canonical.parts().authority_endpoint().as_deref(), Some("guardian.example.com:4101"));
}
