use alloc::{vec, vec::Vec};
use core::{
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  time::Duration,
};

use fraktor_utils_rs::core::{
  collections::queue::capabilities::{QueueCapabilityRegistry, QueueCapabilitySet},
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
  time::TimerInstant,
  timing::{DelayFuture, DelayProvider},
};

use super::ActorSystem;
use crate::core::{
  actor_prim::{
    Actor, ActorCell,
    actor_path::{ActorPath, ActorPathParts, ActorPathScheme},
    actor_ref::ActorRefGeneric,
  },
  dispatcher::{DispatchError, DispatchExecutor, DispatchSharedGeneric, DispatcherConfig},
  error::ActorError,
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  lifecycle::LifecycleStage,
  messaging::SystemMessage,
  props::{MailboxConfig, MailboxRequirement, Props},
  scheduler::{
    AutoDriverMetadata, AutoProfileKind, SchedulerConfig, SchedulerContext, TickDriverId, TickDriverKind,
    TickDriverMetadata,
  },
  system::{ActorRefProvider, ActorRefResolveError, ActorSystemConfig, RemotingConfig},
};

struct TestActor;

impl Actor for TestActor {
  fn receive(
    &mut self,
    _context: &mut crate::core::actor_prim::ActorContextGeneric<'_, NoStdToolbox>,
    _message: crate::core::messaging::AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), crate::core::error::ActorError> {
    Ok(())
  }
}

struct SpawnRecorderActor {
  log: ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl SpawnRecorderActor {
  fn new(log: ArcShared<NoStdMutex<Vec<&'static str>>>) -> Self {
    Self { log }
  }
}

impl Actor for SpawnRecorderActor {
  fn pre_start(
    &mut self,
    _ctx: &mut crate::core::actor_prim::ActorContextGeneric<'_, NoStdToolbox>,
  ) -> Result<(), crate::core::error::ActorError> {
    self.log.lock().push("pre_start");
    Ok(())
  }

  fn receive(
    &mut self,
    _context: &mut crate::core::actor_prim::ActorContextGeneric<'_, NoStdToolbox>,
    _message: crate::core::messaging::AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), crate::core::error::ActorError> {
    self.log.lock().push("receive");
    Ok(())
  }
}

struct FailingStartActor;

impl Actor for FailingStartActor {
  fn receive(
    &mut self,
    _context: &mut crate::core::actor_prim::ActorContextGeneric<'_, NoStdToolbox>,
    _message: crate::core::messaging::AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), crate::core::error::ActorError> {
    Ok(())
  }

  fn pre_start(
    &mut self,
    _ctx: &mut crate::core::actor_prim::ActorContextGeneric<'_, NoStdToolbox>,
  ) -> Result<(), crate::core::error::ActorError> {
    Err(crate::core::error::ActorError::recoverable("boom"))
  }
}

struct LifecycleEventWatcher {
  stages: ArcShared<NoStdMutex<Vec<LifecycleStage>>>,
}

impl LifecycleEventWatcher {
  fn new(stages: ArcShared<NoStdMutex<Vec<LifecycleStage>>>) -> Self {
    Self { stages }
  }
}

impl EventStreamSubscriber<NoStdToolbox> for LifecycleEventWatcher {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    if let EventStreamEvent::Lifecycle(lifecycle) = event {
      self.stages.lock().push(lifecycle.stage());
    }
  }
}

struct NoopExecutor;

impl NoopExecutor {
  const fn new() -> Self {
    Self
  }
}

impl DispatchExecutor<NoStdToolbox> for NoopExecutor {
  fn execute(&self, _dispatcher: DispatchSharedGeneric<NoStdToolbox>) -> Result<(), DispatchError> {
    Ok(())
  }
}

#[test]
fn actor_system_new_empty() {
  let system = ActorSystem::new_empty();
  assert!(!system.state().is_terminated());
}

#[test]
fn actor_system_from_state() {
  let state = crate::core::system::system_state::SystemState::new();
  let system = ActorSystem::from_state(ArcShared::new(state));
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
  system.emit_log(crate::core::logging::LogLevel::Info, "test message", Some(pid));
}

#[test]
fn actor_system_when_terminated() {
  let system = ActorSystem::new_empty();
  let future = system.when_terminated();
  assert!(!future.is_ready());
}

#[test]
fn actor_system_reports_tick_driver_snapshot() {
  let system = ActorSystem::new_empty();
  let ctx = ArcShared::new(SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default()));
  system.state().install_scheduler_context(ctx.clone());

  let driver_id = TickDriverId::new(99);
  let resolution = Duration::from_millis(5);
  let instant = TimerInstant::from_ticks(1, resolution);
  let metadata = TickDriverMetadata::new(driver_id, instant);
  let auto = Some(AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id, resolution });

  ctx.record_driver_metadata(TickDriverKind::Auto, resolution, metadata, auto.clone());

  let snapshot = system.tick_driver_snapshot().expect("tick driver snapshot");
  assert_eq!(snapshot.metadata.driver_id, driver_id);
  assert_eq!(snapshot.kind, TickDriverKind::Auto);
  assert_eq!(snapshot.resolution, resolution);
  assert_eq!(snapshot.auto.as_ref().map(|meta| meta.profile), auto.as_ref().map(|meta| meta.profile));
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
  let props = Props::from_fn(|| TestActor).with_mailbox(mailbox);

  let result = system.spawn_child(parent_pid, &props);
  assert!(matches!(result, Err(crate::core::spawn::SpawnError::InvalidProps(_))));
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
  let props = Props::from_fn(|| TestActor).with_mailbox(mailbox);

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
  assert!(matches!(result, Err(crate::core::spawn::SpawnError::InvalidProps(_))));
}

#[test]
fn spawn_child_resolves_mailbox_id_with_requirements() {
  use fraktor_utils_rs::core::collections::queue::capabilities::{QueueCapabilityRegistry, QueueCapabilitySet};

  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let parent_name = system.state().assign_name(None, Some("parent"), parent_pid).expect("parent name");
  let parent_cell = ActorCell::create(system.state(), parent_pid, None, parent_name, &Props::from_fn(|| TestActor))
    .expect("create actor cell");
  system.state().register_cell(parent_cell);

  let registry = QueueCapabilityRegistry::new(QueueCapabilitySet::defaults().with_deque(false));
  let constrained =
    MailboxConfig::default().with_requirement(MailboxRequirement::requires_deque()).with_capabilities(registry);
  system.extended().mailboxes().register("constrained", constrained).expect("register mailbox");

  let props = Props::from_fn(|| TestActor)
    .with_mailbox_id("constrained")
    .with_mailbox_requirement(MailboxRequirement::for_stash());

  let result = system.spawn_child(parent_pid, &props);
  assert!(matches!(result, Err(crate::core::spawn::SpawnError::InvalidProps(_))));
}

#[test]
fn actor_system_spawn_without_guardian() {
  let system = ActorSystem::new_empty();
  let props = Props::from_fn(|| TestActor);

  let result = system.spawn(&props);
  assert!(result.is_err());
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
  let system = ActorSystem::new_empty();
  let log: ArcShared<NoStdMutex<Vec<&'static str>>> = ArcShared::new(NoStdMutex::new(Vec::new()));

  // Register NoopExecutor as "noop" dispatcher
  let noop_config = DispatcherConfig::from_executor(ArcShared::new(NoopExecutor::new()));
  system.state().dispatchers().register("noop", noop_config.clone()).expect("register noop dispatcher");

  let props = Props::from_fn({
    let log = log.clone();
    move || SpawnRecorderActor::new(log.clone())
  })
  .with_dispatcher_id("noop"); // Use dispatcher_id instead of with_dispatcher

  let child = system.spawn_with_parent(None, &props).expect("spawn succeeds");
  assert!(log.lock().is_empty());
  assert!(system.state().cell(&child.pid()).is_some());
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
    | Err(crate::core::spawn::SpawnError::InvalidProps(reason)) => {
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
  let system = ActorSystem::new_empty();
  let props =
    Props::from_fn(|| TestActor).with_dispatcher(DispatcherConfig::from_executor(ArcShared::new(NoopExecutor::new())));
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
fn actor_system_scheduler_context_handles_delays() {
  let props = Props::from_fn(|| TestActor);
  let tick_driver = crate::core::scheduler::TickDriverConfig::manual(crate::core::scheduler::ManualTestDriver::new());
  let system = ActorSystem::new(&props, tick_driver).expect("system");
  let provider = system.delay_provider().expect("delay provider");
  let mut future = provider.delay(Duration::from_millis(1));
  assert!(matches!(poll_delay(&mut future), Poll::Pending));

  let context = system.scheduler_context().expect("scheduler context");
  let scheduler = context.scheduler();
  {
    let mut guard = scheduler.lock();
    guard.run_for_test(1);
  }

  assert!(matches!(poll_delay(&mut future), Poll::Ready(())));
}

#[test]
fn actor_system_terminate_runs_scheduler_tasks() {
  let props = Props::from_fn(|| TestActor);
  let tick_driver = crate::core::scheduler::TickDriverConfig::manual(crate::core::scheduler::ManualTestDriver::new());
  let system = ActorSystem::new(&props, tick_driver).expect("system");
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  {
    let context = system.scheduler_context().expect("context");
    let scheduler = context.scheduler();
    let mut guard = scheduler.lock();
    let task = RecordingShutdownTask { log: log.clone() };
    guard.register_on_close(ArcShared::new(task), crate::core::scheduler::TaskRunPriority::User).expect("register");
  }

  system.terminate().expect("terminate");

  assert_eq!(log.lock().as_slice(), &["shutdown"]);
}

struct RecordingShutdownTask {
  log: ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl crate::core::scheduler::TaskRunOnClose for RecordingShutdownTask {
  fn run(&self) -> Result<(), crate::core::scheduler::TaskRunError> {
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
fn actor_system_installs_scheduler_context() {
  let props = Props::from_fn(|| TestActor);
  let tick_driver = crate::core::scheduler::TickDriverConfig::manual(crate::core::scheduler::ManualTestDriver::new());
  let system = ActorSystem::new(&props, tick_driver).expect("actor system");
  let provider = system.delay_provider().expect("delay provider");
  let mut future = provider.delay(Duration::from_millis(1));
  assert!(matches!(poll_delay_future(&mut future), Poll::Pending));

  let context = system.scheduler_context().expect("scheduler context");
  {
    let scheduler = context.scheduler();
    let mut guard = scheduler.lock();
    guard.run_for_test(1);
  }

  assert!(matches!(poll_delay_future(&mut future), Poll::Ready(())));
}

#[test]
fn lifecycle_events_cover_restart_transitions() {
  let system = ActorSystem::new_empty();
  let stages: ArcShared<NoStdMutex<Vec<LifecycleStage>>> = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber_impl = ArcShared::new(LifecycleEventWatcher::new(stages.clone()));
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl;
  let _subscription = system.subscribe_event_stream(&subscriber);

  let props = Props::from_fn(|| TestActor);
  let child = system.spawn_with_parent(None, &props).expect("spawn succeeds");
  let pid = child.pid();

  system.state().send_system_message(pid, SystemMessage::Recreate).expect("recreate enqueued");

  let snapshot = stages.lock().clone();
  assert_eq!(snapshot, vec![LifecycleStage::Started, LifecycleStage::Stopped, LifecycleStage::Restarted]);
}

struct DummyActorRefProvider {
  last_path: ArcShared<NoStdMutex<Option<ActorPath>>>,
}

impl DummyActorRefProvider {
  fn new(last_path: ArcShared<NoStdMutex<Option<ActorPath>>>) -> Self {
    Self { last_path }
  }
}

impl ActorRefProvider<NoStdToolbox> for DummyActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    &[ActorPathScheme::FraktorTcp]
  }

  fn actor_ref(&self, path: ActorPath) -> Result<ActorRefGeneric<NoStdToolbox>, ActorError> {
    *self.last_path.lock() = Some(path.clone());
    Ok(ActorRefGeneric::null())
  }
}

#[test]
fn resolve_actor_ref_injects_canonical_authority() {
  let system = ActorSystem::new_empty();
  let remoting = RemotingConfig::default().with_canonical_host("example.com").with_canonical_port(2552);
  let config = ActorSystemConfig::default().with_remoting_config(remoting);
  system.state().apply_actor_system_config(&config);

  let recorded = ArcShared::new(NoStdMutex::new(None));
  let provider = ArcShared::new(DummyActorRefProvider::new(recorded.clone()));
  system.extended().register_actor_ref_provider(&provider);

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
  let system = ActorSystem::new_empty();
  let remoting = RemotingConfig::default().with_canonical_host("example.com").with_canonical_port(2552);
  let config = ActorSystemConfig::default().with_remoting_config(remoting);
  system.state().apply_actor_system_config(&config);

  let path = ActorPath::root().child("svc");
  let result = system.resolve_actor_ref(path);

  assert!(matches!(result, Err(ActorRefResolveError::ProviderMissing)));
}
