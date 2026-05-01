extern crate std;

use core::{num::NonZeroUsize, time::Duration};
use std::{boxed::Box, thread, time::Instant};

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, ChildRef, Pid,
    error::ActorError,
    messaging::{AnyMessageView, system_message::SystemMessage},
    props::Props,
    scheduler::{SchedulerConfig, SchedulerHandle},
    setup::ActorSystemConfig,
  },
  dispatch::dispatcher::{
    DEFAULT_DISPATCHER_ID, DefaultDispatcherFactory, DispatcherConfig, ExecuteError, Executor, ExecutorShared,
    MessageDispatcherFactory, MessageDispatcherShared, TrampolineState,
  },
  system::{ActorSystem, remote::RemotingConfig},
};
use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use super::{
  ActorMaterializer, DownstreamCancellationBoundary, GraphKillSwitchCommandTarget, MaterializedStreamResources,
};
use crate::core::{
  DemandTracker, DynValue, KillSwitchCommandTarget, KillSwitchState, KillSwitchStateHandle, SharedKillSwitch,
  SinkDecision, SinkLogic, SourceLogic, StreamError,
  dsl::{Flow, GraphDsl, GraphDslBuilder, Sink, Source},
  r#impl::{
    fusing::StreamBufferConfig,
    interpreter::IslandBoundaryShared,
    materialization::{Stream, StreamIslandCommand, StreamIslandDriveGate, StreamShared, StreamState},
  },
  materialization::{
    ActorMaterializerConfig, Completion, DriveOutcome, KeepRight, MaterializerLifecycleState, RunnableGraph,
    StreamNotUsed, empty_downstream_cancellation_control_plane,
  },
  stage::StageKind,
};

struct StreamIslandActorDiagnostic {
  actor_pid:            Pid,
  actor_name:           String,
  dispatcher_id:        String,
  cancel_command_count: u32,
}

impl StreamIslandActorDiagnostic {
  fn new(actor_pid: Pid, actor_name: String, dispatcher_id: String, cancel_command_count: u32) -> Self {
    Self { actor_pid, actor_name, dispatcher_id, cancel_command_count }
  }

  const fn actor_pid(&self) -> Pid {
    self.actor_pid
  }

  fn actor_name(&self) -> &str {
    &self.actor_name
  }

  fn dispatcher_id(&self) -> &str {
    &self.dispatcher_id
  }

  const fn cancel_command_count(&self) -> u32 {
    self.cancel_command_count
  }
}

impl ActorMaterializer {
  const fn new_without_system(config: ActorMaterializerConfig) -> Self {
    Self {
      system: None,
      config,
      state: MaterializerLifecycleState::Idle,
      total_materialized: 0,
      streams: Vec::new(),
      materialized: Vec::new(),
    }
  }

  fn island_actor_diagnostics_for_test(&self) -> Result<Vec<StreamIslandActorDiagnostic>, StreamError> {
    let system = self.system()?;
    let state = system.state();
    let mut diagnostics = Vec::new();

    for resources in &self.materialized {
      for actor in &resources.island_actors {
        let pid = actor.pid();
        let cell = state.cell(&pid).ok_or(StreamError::Failed)?;
        diagnostics.push(StreamIslandActorDiagnostic::new(
          pid,
          cell.name().to_owned(),
          cell.new_dispatcher_shared().id(),
          resources.downstream_cancellation_control_plane.lock().cancel_command_count_for_actor(pid),
        ));
      }
    }

    Ok(diagnostics)
  }
}

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct DriveRecordingActor {
  drives: ArcShared<SpinSyncMutex<u32>>,
}

struct CommandRecordingActor {
  shutdowns: ArcShared<SpinSyncMutex<u32>>,
  aborts:    ArcShared<SpinSyncMutex<u32>>,
}

impl DriveRecordingActor {
  const fn new(drives: ArcShared<SpinSyncMutex<u32>>) -> Self {
    Self { drives }
  }
}

impl CommandRecordingActor {
  const fn new(shutdowns: ArcShared<SpinSyncMutex<u32>>, aborts: ArcShared<SpinSyncMutex<u32>>) -> Self {
    Self { shutdowns, aborts }
  }
}

impl Actor for DriveRecordingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if matches!(message.downcast_ref::<StreamIslandCommand>(), Some(StreamIslandCommand::Drive)) {
      let mut drives = self.drives.lock();
      *drives = drives.saturating_add(1);
    }
    Ok(())
  }
}

impl Actor for CommandRecordingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    match message.downcast_ref::<StreamIslandCommand>() {
      | Some(StreamIslandCommand::Shutdown) => {
        let mut shutdowns = self.shutdowns.lock();
        *shutdowns = shutdowns.saturating_add(1);
      },
      | Some(StreamIslandCommand::Abort(_)) => {
        let mut aborts = self.aborts.lock();
        *aborts = aborts.saturating_add(1);
      },
      | _ => {},
    }
    Ok(())
  }
}

struct FailOnStartSinkLogic;

impl SinkLogic for FailOnStartSinkLogic {
  fn on_start(&mut self, _demand: &mut DemandTracker) -> Result<(), StreamError> {
    Err(StreamError::Failed)
  }

  fn on_push(&mut self, _input: DynValue, _demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_error(&mut self, _error: StreamError) {}
}

struct CancelAwareSourceLogic {
  cancel_count: ArcShared<SpinSyncMutex<u32>>,
}

struct CancelFailingSourceLogic;

struct DrainOnShutdownFailingSourceLogic {
  error: StreamError,
}

struct ShutdownFailingSourceLogic {
  error: StreamError,
}

struct ShutdownFailingEndlessSourceLogic {
  next:  u32,
  error: StreamError,
}

struct DrainOnShutdownCancelFailingPendingSourceLogic;

struct DrainOnShutdownPendingSourceLogic;

impl CancelAwareSourceLogic {
  const fn new(cancel_count: ArcShared<SpinSyncMutex<u32>>) -> Self {
    Self { cancel_count }
  }
}

impl DrainOnShutdownFailingSourceLogic {
  const fn new(error: StreamError) -> Self {
    Self { error }
  }
}

impl ShutdownFailingSourceLogic {
  const fn new(error: StreamError) -> Self {
    Self { error }
  }
}

impl ShutdownFailingEndlessSourceLogic {
  const fn new(error: StreamError) -> Self {
    Self { next: 0, error }
  }
}

impl SourceLogic for CancelAwareSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(StreamError::WouldBlock)
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    let mut count = self.cancel_count.lock();
    *count = count.saturating_add(1);
    Ok(())
  }
}

impl SourceLogic for CancelFailingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(StreamError::WouldBlock)
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    Err(StreamError::Failed)
  }
}

impl SourceLogic for DrainOnShutdownFailingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(self.error.clone())
  }

  fn should_drain_on_shutdown(&self) -> bool {
    true
  }
}

impl SourceLogic for DrainOnShutdownPendingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(StreamError::WouldBlock)
  }

  fn should_drain_on_shutdown(&self) -> bool {
    true
  }
}

impl SourceLogic for DrainOnShutdownCancelFailingPendingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(StreamError::WouldBlock)
  }

  fn should_drain_on_shutdown(&self) -> bool {
    true
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    Err(StreamError::Failed)
  }
}

impl SourceLogic for ShutdownFailingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(StreamError::WouldBlock)
  }

  fn on_shutdown(&mut self) -> Result<(), StreamError> {
    Err(self.error.clone())
  }
}

impl SourceLogic for ShutdownFailingEndlessSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    self.next = self.next.saturating_add(1);
    Ok(Some(Box::new(self.next)))
  }

  fn on_shutdown(&mut self) -> Result<(), StreamError> {
    Err(self.error.clone())
  }
}

struct EndlessSourceLogic {
  next: u32,
}

struct PendingSourceLogic;

impl EndlessSourceLogic {
  const fn new() -> Self {
    Self { next: 0 }
  }
}

impl SourceLogic for EndlessSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    self.next = self.next.saturating_add(1);
    Ok(Some(Box::new(self.next)))
  }
}

impl SourceLogic for PendingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(StreamError::WouldBlock)
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  ActorSystem::create_with_config(&props, config).expect("system should build")
}

struct InlineExec;

impl Executor for InlineExec {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    task();
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn nz(value: usize) -> NonZeroUsize {
  NonZeroUsize::new(value).expect("non-zero")
}

fn inline_executor_shared() -> ExecutorShared {
  ExecutorShared::new(Box::new(InlineExec), TrampolineState::new())
}

fn build_system_with_dispatcher(dispatcher_id: &'static str) -> (ActorSystem, MessageDispatcherShared) {
  let executor = inline_executor_shared();
  let settings = DispatcherConfig::new(dispatcher_id, nz(8), None, Duration::from_secs(1));
  let factory = DefaultDispatcherFactory::new(&settings, executor);
  let dispatcher = factory.dispatcher();
  let configurator: Box<dyn MessageDispatcherFactory> = Box::new(factory);
  let configurator_handle: ArcShared<Box<dyn MessageDispatcherFactory>> = ArcShared::new(configurator);

  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler)
    .with_dispatcher_factory(dispatcher_id, configurator_handle);
  (ActorSystem::create_with_config(&props, config).expect("system should build"), dispatcher)
}

fn system_child_names(system: &ActorSystem) -> Vec<String> {
  let state = system.state();
  let guardian_pid = state.system_guardian_pid().expect("system guardian should exist");
  state
    .child_pids(guardian_pid)
    .into_iter()
    .filter_map(|pid| state.cell(&pid).map(|cell| cell.name().to_owned()))
    .collect()
}

fn scheduler_job_count(system: &ActorSystem) -> usize {
  system.scheduler().with_read(|scheduler| scheduler.dump().jobs().len())
}

fn resources_with_unknown_tick() -> MaterializedStreamResources {
  let mut resources = MaterializedStreamResources::new(Vec::new(), empty_downstream_cancellation_control_plane());
  resources.tick_handles.push(SchedulerHandle::new(u64::MAX));
  resources
}

fn running_stream_from_graph<Mat>(graph: RunnableGraph<Mat>) -> StreamShared {
  let (plan, _materialized) = graph.into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("stream should start");
  StreamShared::new(stream)
}

fn drive_count(drives: &ArcShared<SpinSyncMutex<u32>>) -> u32 {
  *drives.lock()
}

fn wait_for_drive_count(drives: &ArcShared<SpinSyncMutex<u32>>, expected: u32) {
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if drive_count(drives) == expected {
      return;
    }
    thread::yield_now();
  }
  assert_eq!(drive_count(drives), expected);
}

fn wait_for_counter(counter: &ArcShared<SpinSyncMutex<u32>>, expected: u32) {
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if *counter.lock() == expected {
      return;
    }
    thread::yield_now();
  }
  assert_eq!(*counter.lock(), expected);
}

fn wait_for_actor_cell_removed(system: &ActorSystem, pid: Pid) {
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if system.state().cell(&pid).is_none() {
      return;
    }
    thread::yield_now();
  }
  assert!(system.state().cell(&pid).is_none());
}

fn stopped_system_actor(system: &ActorSystem) -> ChildRef {
  let props = Props::from_fn(|| GuardianActor);
  let actor = system.extended().spawn_system_actor(&props).expect("actor should spawn");
  let actor_pid = actor.pid();
  actor.stop().expect("actor should stop");
  wait_for_actor_cell_removed(system, actor_pid);
  actor
}

fn upstream_cancel_command_count(materializer: &ActorMaterializer) -> u32 {
  let diagnostics: Vec<StreamIslandActorDiagnostic> =
    materializer.island_actor_diagnostics_for_test().expect("island actor diagnostics should be available");
  diagnostics.first().expect("upstream island actor should be diagnosable").cancel_command_count()
}

fn wait_for_upstream_cancel_command_count(materializer: &ActorMaterializer, expected: u32) {
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if upstream_cancel_command_count(materializer) == expected {
      return;
    }
    thread::yield_now();
  }
  assert_eq!(upstream_cancel_command_count(materializer), expected);
}

fn all_streams_share_kill_switch_state(
  materializer: &ActorMaterializer,
  expected_state: &KillSwitchStateHandle,
) -> bool {
  materializer.streams().iter().all(|stream| {
    let actual_state = stream.with_read(|stream| stream.kill_switch_state());
    ArcShared::ptr_eq(&actual_state, expected_state)
  })
}

fn drive_registered_streams_until_all_terminal(materializer: &ActorMaterializer) {
  for _ in 0..64 {
    for stream in materializer.streams() {
      match stream.drive() {
        | DriveOutcome::Progressed | DriveOutcome::Idle => {},
      }
    }
    if materializer.streams().iter().all(|stream| stream.state().is_terminal()) {
      return;
    }
  }
}

fn drive_registered_streams(materializer: &ActorMaterializer, attempts: usize) {
  for _ in 0..attempts {
    for stream in materializer.streams() {
      match stream.drive() {
        | DriveOutcome::Progressed | DriveOutcome::Idle => {},
      }
    }
  }
}

fn wait_for_all_streams_to_reach_state(materializer: &ActorMaterializer, expected: StreamState) {
  let deadline = Instant::now() + Duration::from_secs(2);
  while Instant::now() < deadline {
    if materializer.streams().iter().all(|stream| stream.state() == expected) {
      return;
    }
    thread::yield_now();
  }
  assert!(materializer.streams().iter().all(|stream| stream.state() == expected));
}

#[test]
fn start_fails_without_actor_system() {
  let mut materializer = ActorMaterializer::new_without_system(ActorMaterializerConfig::default());
  let result = materializer.start();
  assert!(matches!(result, Err(StreamError::ActorSystemMissing)));
}

#[test]
fn materialize_fails_without_actor_system() {
  let mut materializer = ActorMaterializer::new_without_system(ActorMaterializerConfig::default());
  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);

  assert!(matches!(result, Err(StreamError::MaterializerNotStarted)));
  assert_eq!(materializer.snapshot().total_materialized(), 0);
}

#[test]
fn materialize_requires_start() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);
  assert!(matches!(result, Err(StreamError::MaterializerNotStarted)));
}

#[test]
fn shutdown_requires_start() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());

  let result = materializer.shutdown();

  assert!(matches!(result, Err(StreamError::MaterializerNotStarted)));
  assert_eq!(materializer.lifecycle_state(), MaterializerLifecycleState::Idle);
}

#[test]
fn start_fails_when_already_running() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");

  let result = materializer.start();

  assert!(matches!(result, Err(StreamError::MaterializerAlreadyStarted)));
}

#[test]
fn actor_materializer_drives_stream() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");
  let graph =
    Source::single(1_u32).map(|value| value + 1).into_mat(Sink::fold(0_u32, |acc, value| acc + value), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if matches!(materialized.materialized().poll(), Completion::Ready(_)) {
      break;
    }
    std::thread::yield_now();
  }
  assert_eq!(materialized.stream().state(), StreamState::Completed);
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(2)));
}

#[test]
fn shutdown_blocks_materialize() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
  materializer.shutdown().expect("shutdown");
  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);
  assert!(matches!(result, Err(StreamError::MaterializerStopped)));
}

#[test]
fn shutdown_after_shutdown_returns_stopped_error() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
  materializer.shutdown().expect("shutdown");

  let result = materializer.shutdown();

  assert!(matches!(result, Err(StreamError::MaterializerStopped)));
}

#[test]
fn start_after_shutdown_returns_stopped_error() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
  materializer.shutdown().expect("shutdown");

  let result = materializer.start();

  assert!(matches!(result, Err(StreamError::MaterializerStopped)));
}

#[test]
fn start_with_remoting_config() {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let remoting = RemotingConfig::default().with_canonical_host("127.0.0.1").with_canonical_port(2552);
  let config =
    ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler).with_remoting_config(remoting);
  let system = ActorSystem::create_with_config(&props, config).expect("system should build");
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
}

#[test]
fn start_does_not_spawn_legacy_stream_drive_actor() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system.clone(), ActorMaterializerConfig::default());

  materializer.start().expect("start");

  let names = system_child_names(&system);
  assert!(!names.iter().any(|name| name == "stream-drive"));
}

#[test]
fn materialize_spawns_one_island_actor_per_async_island() {
  let system = build_system();
  let child_count_before_start = system_child_names(&system).len();
  let mut materializer = ActorMaterializer::new(
    system.clone(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("start");

  let graph = Source::single(1_u32).r#async().into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  let names = system_child_names(&system);
  assert_eq!(names.len(), child_count_before_start + 2);
  assert!(!names.iter().any(|name| name == "stream-drive"));
}

#[test]
fn async_with_dispatcher_assigns_dispatcher_to_downstream_island_actor() {
  let (system, dispatcher) = build_system_with_dispatcher("stream-custom-dispatcher");
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");
  assert_eq!(dispatcher.inhabitants(), 0);

  let graph = Source::single(1_u32).async_with_dispatcher("stream-custom-dispatcher").into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(dispatcher.inhabitants(), 1);
}

#[test]
fn flow_async_with_dispatcher_assigns_dispatcher_to_downstream_island_actor() {
  let (system, dispatcher) = build_system_with_dispatcher("stream-custom-dispatcher");
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");
  assert_eq!(dispatcher.inhabitants(), 0);

  let graph = Source::single(1_u32)
    .via(Flow::new().map(|value: u32| value).async_with_dispatcher("stream-custom-dispatcher"))
    .into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(dispatcher.inhabitants(), 1);
}

#[test]
fn async_with_dispatcher_exposes_downstream_island_actor_dispatcher_in_test_diagnostic() {
  let (system, _dispatcher) = build_system_with_dispatcher("stream-custom-dispatcher");
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  let graph = Source::single(1_u32).async_with_dispatcher("stream-custom-dispatcher").into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  let diagnostics: Vec<StreamIslandActorDiagnostic> =
    materializer.island_actor_diagnostics_for_test().expect("island actor diagnostics should be available");
  assert_eq!(diagnostics.len(), 2);
  let custom_dispatcher_actor = diagnostics
    .iter()
    .find(|diagnostic| diagnostic.dispatcher_id() == "stream-custom-dispatcher")
    .expect("custom dispatcher island actor should be diagnosable");

  assert!(custom_dispatcher_actor.actor_name().starts_with("stream-island-"));
  assert!(custom_dispatcher_actor.actor_pid().value() > 0);
}

#[test]
fn async_without_dispatcher_uses_default_dispatcher_for_all_island_actors() {
  let system = build_system();
  let default_dispatcher =
    system.state().resolve_dispatcher(DEFAULT_DISPATCHER_ID).expect("default dispatcher should resolve");
  let inhabitants_before_start = default_dispatcher.inhabitants();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  let graph = Source::single(1_u32).r#async().into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(default_dispatcher.inhabitants(), inhabitants_before_start + 2);
}

#[test]
fn materialize_fails_without_default_dispatcher_fallback_when_dispatcher_is_missing() {
  let system = build_system();
  let default_dispatcher =
    system.state().resolve_dispatcher(DEFAULT_DISPATCHER_ID).expect("default dispatcher should resolve");
  let default_inhabitants_before = default_dispatcher.inhabitants();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  let graph =
    Source::single(1_u32).async_with_dispatcher("missing-stream-dispatcher").into_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);

  assert!(matches!(result, Err(StreamError::Failed)));
  assert_eq!(default_dispatcher.inhabitants(), default_inhabitants_before);
  assert_eq!(materializer.snapshot().total_materialized(), 0);
}

#[test]
fn materialize_rolls_back_spawned_island_actor_and_tick_when_later_dispatcher_is_missing() {
  let system = build_system();
  let child_count_before = system_child_names(&system).len();
  let scheduler_jobs_before = scheduler_job_count(&system);
  let mut materializer = ActorMaterializer::new(
    system.clone(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("start");

  let graph =
    Source::single(1_u32).async_with_dispatcher("missing-stream-dispatcher").into_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);

  assert!(matches!(result, Err(StreamError::Failed)));
  assert_eq!(system_child_names(&system).len(), child_count_before);
  assert_eq!(scheduler_job_count(&system), scheduler_jobs_before);
  assert!(materializer.streams().is_empty());
  assert_eq!(materializer.snapshot().total_materialized(), 0);
}

#[test]
fn materialize_cancels_started_streams_when_later_dispatcher_is_missing() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");
  let cancel_count = ArcShared::new(SpinSyncMutex::new(0_u32));

  let source = Source::<u32, _>::from_logic(StageKind::Custom, CancelAwareSourceLogic::new(cancel_count.clone()));
  let graph = source.async_with_dispatcher("missing-stream-dispatcher").into_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);

  assert!(matches!(result, Err(StreamError::Failed)));
  assert_eq!(*cancel_count.lock(), 1);
}

#[test]
fn configure_downstream_cancellation_control_plane_groups_routes_by_upstream_island() {
  let system = build_system();
  let props = Props::from_fn(|| GuardianActor);
  let upstream_actor = system.extended().spawn_system_actor(&props).expect("upstream actor should spawn");
  let first_downstream_actor =
    system.extended().spawn_system_actor(&props).expect("first downstream actor should spawn");
  let second_downstream_actor =
    system.extended().spawn_system_actor(&props).expect("second downstream actor should spawn");
  let island_actors = vec![upstream_actor.clone(), first_downstream_actor, second_downstream_actor];
  let upstream_stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic).into_mat(Sink::ignore(), KeepRight),
  );
  let first_downstream_stream = running_stream_from_graph(Source::single(1_u32).into_mat(Sink::ignore(), KeepRight));
  let second_downstream_stream = running_stream_from_graph(Source::single(2_u32).into_mat(Sink::ignore(), KeepRight));
  let streams = vec![upstream_stream, first_downstream_stream, second_downstream_stream];
  let first_boundary = IslandBoundaryShared::new(1);
  let second_boundary = IslandBoundaryShared::new(1);
  let control_plane = empty_downstream_cancellation_control_plane();
  let boundaries = vec![
    DownstreamCancellationBoundary::new(0, 1, first_boundary.clone()),
    DownstreamCancellationBoundary::new(0, 2, second_boundary.clone()),
  ];

  ActorMaterializer::configure_downstream_cancellation_control_plane(
    boundaries,
    &island_actors,
    &streams,
    &control_plane,
  )
  .expect("control plane should configure");

  first_boundary.cancel_downstream();
  second_boundary.cancel_downstream();
  let reserved = control_plane.lock().reserve_cancellation_targets();

  assert_eq!(reserved.len(), 1);
  assert_eq!(reserved[0].actor_pid(), upstream_actor.pid());
}

#[test]
fn build_materialized_resources_rolls_back_when_cancellation_boundary_is_invalid() {
  let system = build_system();
  let child_count_before = system_child_names(&system).len();
  let stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic).into_mat(Sink::ignore(), KeepRight),
  );
  let control_plane = empty_downstream_cancellation_control_plane();
  let boundaries = vec![DownstreamCancellationBoundary::new(0, 1, IslandBoundaryShared::new(1))];

  let result = ActorMaterializer::build_materialized_resources(
    &system,
    vec![stream.clone()],
    vec![None],
    Duration::from_millis(1),
    boundaries,
    &control_plane,
  );

  assert_eq!(result.err(), Some(StreamError::InvalidConnection));
  assert_eq!(stream.state(), StreamState::Cancelled);
  assert_eq!(system_child_names(&system).len(), child_count_before);
}

#[test]
fn build_materialized_resources_rolls_back_when_tick_scheduling_fails() {
  let system = build_system();
  let child_count_before = system_child_names(&system).len();
  let stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic).into_mat(Sink::ignore(), KeepRight),
  );
  let shutdown_summary = system.scheduler().with_write(|scheduler| scheduler.shutdown());
  assert_eq!(shutdown_summary.failed_tasks, 0);
  let control_plane = empty_downstream_cancellation_control_plane();

  let result = ActorMaterializer::build_materialized_resources(
    &system,
    vec![stream.clone()],
    vec![None],
    Duration::from_millis(1),
    Vec::new(),
    &control_plane,
  );

  assert_eq!(result.err(), Some(StreamError::Failed));
  assert_eq!(stream.state(), StreamState::Cancelled);
  assert_eq!(system_child_names(&system).len(), child_count_before);
}

#[test]
fn cancel_resources_reports_first_stream_cancel_failure() {
  let system = build_system();
  let failing_stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, CancelFailingSourceLogic).into_mat(Sink::ignore(), KeepRight),
  );
  let healthy_stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic).into_mat(Sink::ignore(), KeepRight),
  );
  let resources = MaterializedStreamResources::new(
    vec![failing_stream, healthy_stream.clone()],
    empty_downstream_cancellation_control_plane(),
  );

  let result = ActorMaterializer::cancel_resources(&system, resources, None);

  assert_eq!(result, Err(StreamError::Failed));
  assert_eq!(healthy_stream.state(), StreamState::Cancelled);
}

#[test]
fn cancel_resources_reports_stopped_island_actor_delivery_failure() {
  let system = build_system();
  let actor = stopped_system_actor(&system);
  let mut resources = MaterializedStreamResources::new(Vec::new(), empty_downstream_cancellation_control_plane());
  resources.island_actors.push(actor);

  let result = ActorMaterializer::cancel_resources(&system, resources, None);

  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn materialize_registers_one_scheduler_job_per_island_actor() {
  let system = build_system();
  let scheduler_jobs_before_start = scheduler_job_count(&system);
  let mut materializer = ActorMaterializer::new(
    system.clone(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("start");

  let graph = Source::single(1_u32).r#async().into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(scheduler_job_count(&system), scheduler_jobs_before_start + 2);
}

#[test]
fn send_drive_if_idle_coalesces_while_gate_is_pending() {
  let system = build_system();
  let drives = ArcShared::new(SpinSyncMutex::new(0_u32));
  let actor_drives = drives.clone();
  let props = Props::from_fn(move || DriveRecordingActor::new(actor_drives.clone()));
  let actor = system.extended().spawn_system_actor(&props).expect("actor should spawn");
  let gate = StreamIslandDriveGate::new();

  ActorMaterializer::send_drive_if_idle(&actor, &gate).expect("drive delivery");
  ActorMaterializer::send_drive_if_idle(&actor, &gate).expect("coalesced drive delivery");

  wait_for_drive_count(&drives, 1);
  assert!(!gate.try_mark_pending());
}

#[test]
fn send_drive_if_idle_reports_delivery_failure_and_releases_gate() {
  let system = build_system();
  let props = Props::from_fn(|| GuardianActor);
  let actor = system.extended().spawn_system_actor(&props).expect("actor should spawn");
  let gate = StreamIslandDriveGate::new();

  actor.stop().expect("stop actor");
  wait_for_actor_cell_removed(&system, actor.pid());
  let result = ActorMaterializer::send_drive_if_idle(&actor, &gate);

  assert!(matches!(result, Err(StreamError::Failed)));
  assert!(gate.try_mark_pending());
}

#[test]
fn graph_kill_switch_target_reports_first_delivery_failure() {
  let system = build_system();
  let props = Props::from_fn(|| GuardianActor);
  let actor = system.extended().spawn_system_actor(&props).expect("actor should spawn");
  actor.stop().expect("stop actor");
  wait_for_actor_cell_removed(&system, actor.pid());
  let target = GraphKillSwitchCommandTarget::new(&[actor]);

  let result = target.shutdown();

  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn register_graph_kill_switch_target_sends_shutdown_when_state_already_shutdown() {
  let system = build_system();
  let shutdowns = ArcShared::new(SpinSyncMutex::new(0_u32));
  let aborts = ArcShared::new(SpinSyncMutex::new(0_u32));
  let actor_shutdowns = shutdowns.clone();
  let actor_aborts = aborts.clone();
  let props = Props::from_fn(move || CommandRecordingActor::new(actor_shutdowns.clone(), actor_aborts.clone()));
  let actor = system.extended().spawn_system_actor(&props).expect("actor should spawn");
  let state: KillSwitchStateHandle = ArcShared::new(SpinSyncMutex::new(KillSwitchState::running()));
  assert!(state.lock().request_shutdown().is_some());

  ActorMaterializer::register_graph_kill_switch_target(&state, &[actor]).expect("register kill switch target");

  wait_for_counter(&shutdowns, 1);
  assert_eq!(*aborts.lock(), 0);
}

#[test]
fn register_graph_kill_switch_target_sends_abort_when_state_already_aborted() {
  let system = build_system();
  let shutdowns = ArcShared::new(SpinSyncMutex::new(0_u32));
  let aborts = ArcShared::new(SpinSyncMutex::new(0_u32));
  let actor_shutdowns = shutdowns.clone();
  let actor_aborts = aborts.clone();
  let props = Props::from_fn(move || CommandRecordingActor::new(actor_shutdowns.clone(), actor_aborts.clone()));
  let actor = system.extended().spawn_system_actor(&props).expect("actor should spawn");
  let state: KillSwitchStateHandle = ArcShared::new(SpinSyncMutex::new(KillSwitchState::running()));
  assert!(state.lock().request_abort(StreamError::Failed).is_some());

  ActorMaterializer::register_graph_kill_switch_target(&state, &[actor]).expect("register kill switch target");

  wait_for_counter(&aborts, 1);
  assert_eq!(*shutdowns.lock(), 0);
}

#[test]
fn register_graph_kill_switch_target_or_rollback_reports_delivery_failure() {
  let system = build_system();
  let actor = stopped_system_actor(&system);
  let state: KillSwitchStateHandle = ArcShared::new(SpinSyncMutex::new(KillSwitchState::running()));
  assert!(state.lock().request_shutdown().is_some());
  let mut resources = MaterializedStreamResources::new(Vec::new(), empty_downstream_cancellation_control_plane());
  resources.island_actors.push(actor);

  let result = ActorMaterializer::register_graph_kill_switch_target_or_rollback(&system, resources, &state);

  assert!(result.is_err());
}

#[test]
fn shutdown_stops_island_actors_and_cancels_scheduler_jobs() {
  let system = build_system();
  let child_count_before_start = system_child_names(&system).len();
  let scheduler_jobs_before_start = scheduler_job_count(&system);
  let mut materializer = ActorMaterializer::new(
    system.clone(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("start");
  let graph = Source::single(1_u32).r#async().into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(system_child_names(&system).len(), child_count_before_start + 2);
  assert_eq!(scheduler_job_count(&system), scheduler_jobs_before_start + 2);

  materializer.shutdown().expect("shutdown");

  assert_eq!(system_child_names(&system).len(), child_count_before_start);
  assert_eq!(scheduler_job_count(&system), scheduler_jobs_before_start);
}

#[test]
fn shutdown_succeeds_when_tick_resources_were_already_drained_by_scheduler_shutdown() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(
    system.clone(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("start");
  let graph = Source::single(1_u32).r#async().into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(scheduler_job_count(&system), 2);

  let shutdown_summary = system.scheduler().with_write(|scheduler| scheduler.shutdown());
  assert_eq!(shutdown_summary.failed_tasks, 0);
  let result = materializer.shutdown();

  assert_eq!(result, Ok(()));
}

#[test]
fn rollback_materialized_resources_keeps_primary_error_when_teardown_fails() {
  let system = build_system();
  let resources = resources_with_unknown_tick();
  let primary_error = StreamError::InvalidConnection;

  let error = ActorMaterializer::rollback_materialized_resources(&system, resources, primary_error.clone());

  assert_eq!(error.materialization_primary_failure(), Some(&primary_error));
  assert_eq!(error.materialization_cleanup_failure(), Some(&StreamError::Failed));
}

#[test]
fn shutdown_and_cancel_resources_report_the_same_tick_cleanup_failure() {
  let system = build_system();

  let shutdown_result = ActorMaterializer::shutdown_resources(&system, resources_with_unknown_tick());
  let cancel_result = ActorMaterializer::cancel_resources(&system, resources_with_unknown_tick(), None);

  assert_eq!(shutdown_result, Err(StreamError::Failed));
  assert_eq!(cancel_result, Err(StreamError::Failed));
}

#[test]
fn shutdown_resources_drives_other_streams_even_when_one_shutdown_request_fails() {
  let system = build_system();
  let shutdown_error = StreamError::failed_with_context("shutdown request failed");
  let failing_stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, ShutdownFailingSourceLogic::new(shutdown_error.clone()))
      .into_mat(Sink::ignore(), KeepRight),
  );
  let observed_draining_stream =
    running_stream_from_graph(Source::from_array([1_u32, 2_u32]).into_mat(Sink::seq(), KeepRight));
  let resources = MaterializedStreamResources::new(
    vec![failing_stream, observed_draining_stream.clone()],
    empty_downstream_cancellation_control_plane(),
  );

  let result = ActorMaterializer::shutdown_resources(&system, resources);

  assert_eq!(result, Err(shutdown_error));
  assert_eq!(observed_draining_stream.state(), StreamState::Completed);
}

#[test]
fn shutdown_resources_completes_explicit_unbounded_iterator_without_drain_round_limit() {
  let system = build_system();
  let stream = running_stream_from_graph(Source::from_unbounded_iterator(0_u32..).into_mat(Sink::ignore(), KeepRight));
  let resources = MaterializedStreamResources::new(vec![stream.clone()], empty_downstream_cancellation_control_plane());

  let result = ActorMaterializer::shutdown_resources(&system, resources);

  assert_eq!(result, Ok(()));
  assert_eq!(stream.state(), StreamState::Completed);
}

#[test]
fn shutdown_resources_fails_stream_terminal_when_shutdown_request_fails() {
  let system = build_system();
  let shutdown_error = StreamError::failed_with_context("endless shutdown request failed");
  let failing_stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, ShutdownFailingEndlessSourceLogic::new(shutdown_error.clone()))
      .into_mat(Sink::ignore(), KeepRight),
  );
  let resources =
    MaterializedStreamResources::new(vec![failing_stream.clone()], empty_downstream_cancellation_control_plane());

  let result = ActorMaterializer::shutdown_resources(&system, resources);

  assert_eq!(result, Err(shutdown_error));
  assert_eq!(failing_stream.state(), StreamState::Failed);
}

#[test]
fn shutdown_resources_reports_direct_drain_failure_before_cancel_failure() {
  let system = build_system();
  let stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, DrainOnShutdownCancelFailingPendingSourceLogic)
      .into_mat(Sink::ignore(), KeepRight),
  );
  let resources = MaterializedStreamResources::new(vec![stream], empty_downstream_cancellation_control_plane());

  let result = ActorMaterializer::shutdown_resources(&system, resources);

  assert!(result.is_err());
}

#[test]
fn send_drive_if_idle_releases_gate_when_delivery_fails() {
  let system = build_system();
  let actor = stopped_system_actor(&system);
  let drive_gate = StreamIslandDriveGate::new();

  let result = ActorMaterializer::send_drive_if_idle(&actor, &drive_gate);

  assert_eq!(result, Err(StreamError::Failed));
  assert!(drive_gate.try_mark_pending());
}

#[test]
fn drive_streams_until_terminal_reports_round_limit_when_drain_makes_no_progress() {
  let stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, DrainOnShutdownPendingSourceLogic)
      .into_mat(Sink::ignore(), KeepRight),
  );
  stream.shutdown().expect("shutdown request");

  let result = ActorMaterializer::drive_streams_until_terminal(&[stream]);

  assert!(result.is_err());
}

#[test]
fn drive_actor_owned_streams_until_terminal_reports_invalid_resource_shape() {
  let stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic).into_mat(Sink::ignore(), KeepRight),
  );
  let resources = MaterializedStreamResources::new(vec![stream], empty_downstream_cancellation_control_plane());

  let result = ActorMaterializer::drive_actor_owned_streams_until_terminal(&resources);

  assert_eq!(result, Err(StreamError::InvalidConnection));
}

#[test]
fn drive_actor_owned_streams_until_terminal_reports_direct_drain_round_limit() {
  let system = build_system();
  let stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic).into_mat(Sink::ignore(), KeepRight),
  );
  let drive_gate = StreamIslandDriveGate::new();
  let mut resources = MaterializedStreamResources::new(vec![stream], empty_downstream_cancellation_control_plane());
  resources.island_actors.push(stopped_system_actor(&system));
  resources.drive_gates.push(drive_gate);

  let result = ActorMaterializer::drive_actor_owned_streams_until_terminal(&resources);

  assert!(result.is_err());
}

#[test]
fn shutdown_resources_reports_stopped_actor_and_cancel_failure() {
  let system = build_system();
  let actor = stopped_system_actor(&system);
  let stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, CancelFailingSourceLogic).into_mat(Sink::ignore(), KeepRight),
  );
  let mut resources = MaterializedStreamResources::new(vec![stream], empty_downstream_cancellation_control_plane());
  resources.island_actors.push(actor);

  let result = ActorMaterializer::shutdown_resources(&system, resources);

  assert_eq!(result, Err(StreamError::Failed));
}

#[test]
fn shutdown_reports_resource_teardown_failure_and_clears_materialized_resources() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
  materializer.materialized.push(resources_with_unknown_tick());

  let result = materializer.shutdown();

  assert_eq!(result, Err(StreamError::Failed));
  assert_eq!(materializer.lifecycle_state(), MaterializerLifecycleState::Stopped);
  assert!(materializer.materialized.is_empty());
}

#[test]
fn stream_shutdown_is_idempotent_after_first_request() {
  let stream = running_stream_from_graph(
    Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic).into_mat(Sink::ignore(), KeepRight),
  );

  assert_eq!(stream.shutdown(), Ok(()));
  assert_eq!(stream.shutdown(), Ok(()));
}

// ---------------------------------------------------------------------------
// 診断: lifecycle_state()
// ---------------------------------------------------------------------------

#[test]
fn new_materializer_is_idle() {
  // 準備: 新規 materializer を構築
  let system = build_system();
  let materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());

  // 検証: ライフサイクル状態は Idle
  assert_eq!(materializer.lifecycle_state(), MaterializerLifecycleState::Idle);
}

#[test]
fn started_materializer_is_running() {
  // 準備: start 済みの materializer
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");

  // 検証: ライフサイクル状態は Running
  assert_eq!(materializer.lifecycle_state(), MaterializerLifecycleState::Running);
}

#[test]
fn shutdown_materializer_is_stopped() {
  // Given: a materializer that has been started and shut down
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
  materializer.shutdown().expect("shutdown");

  // Then: lifecycle state is Stopped
  assert_eq!(materializer.lifecycle_state(), MaterializerLifecycleState::Stopped);
}

// ---------------------------------------------------------------------------
// Diagnostics: is_idle() / is_running() / is_stopped()
// ---------------------------------------------------------------------------

#[test]
fn is_idle_true_before_start() {
  // Given: a new materializer
  let system = build_system();
  let materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());

  // Then: is_idle is true, others are false
  assert!(materializer.is_idle());
  assert!(!materializer.is_running());
  assert!(!materializer.is_stopped());
}

#[test]
fn is_running_true_after_start() {
  // Given: a started materializer
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");

  // Then: is_running is true, others are false
  assert!(!materializer.is_idle());
  assert!(materializer.is_running());
  assert!(!materializer.is_stopped());
}

#[test]
fn is_stopped_true_after_shutdown() {
  // Given: a shut down materializer
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
  materializer.shutdown().expect("shutdown");

  // Then: is_stopped is true, others are false
  assert!(!materializer.is_idle());
  assert!(!materializer.is_running());
  assert!(materializer.is_stopped());
}

// ---------------------------------------------------------------------------
// Diagnostics: snapshot()
// ---------------------------------------------------------------------------

#[test]
fn snapshot_reflects_idle_state_with_zero_count() {
  // Given: a new materializer
  let system = build_system();
  let materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());

  // When: taking a snapshot
  let snap = materializer.snapshot();

  // Then: state is Idle and no graphs have been materialized
  assert_eq!(snap.lifecycle_state(), MaterializerLifecycleState::Idle);
  assert_eq!(snap.total_materialized(), 0);
}

#[test]
fn snapshot_reflects_running_state_after_start() {
  // Given: a started materializer (no graphs materialized yet)
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");

  // When: taking a snapshot
  let snap = materializer.snapshot();

  // Then: state is Running and count is still 0
  assert_eq!(snap.lifecycle_state(), MaterializerLifecycleState::Running);
  assert_eq!(snap.total_materialized(), 0);
}

#[test]
fn snapshot_total_materialized_increments_on_successful_materialize() {
  // Given: a running materializer
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  // When: materializing a graph
  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  // Then: total_materialized is 1
  assert_eq!(materializer.snapshot().total_materialized(), 1);

  // When: materializing a second graph
  let graph2 = Source::single(2_u32).into_mat(Sink::head(), KeepRight);
  let _materialized2 = graph2.run(&mut materializer).expect("materialize second");

  // Then: total_materialized is 2
  assert_eq!(materializer.snapshot().total_materialized(), 2);
}

#[test]
fn streams_returns_registered_streams_and_shutdown_clears_them() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(materializer.streams().len(), 1);

  materializer.shutdown().expect("shutdown");

  assert!(materializer.streams().is_empty());
}

#[test]
fn streams_returns_all_island_streams_for_async_boundary() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  let graph = Source::single(1_u32).r#async().into_mat(Sink::head(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(materializer.streams().len(), 2);
  assert!(materializer.streams().iter().any(|s| s.id() == materialized.stream().id()));
}

#[test]
fn materialized_unique_kill_switch_state_is_shared_by_all_islands() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  let graph = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .r#async()
    .into_mat(Sink::ignore(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(materializer.streams().len(), 2);
  let graph_state = materialized.unique_kill_switch().state_handle();
  assert!(all_streams_share_kill_switch_state(&materializer, &graph_state));
}

#[test]
fn materialized_shared_kill_switch_state_is_shared_by_all_islands() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  let graph = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .r#async()
    .into_mat(Sink::ignore(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");

  assert_eq!(materializer.streams().len(), 2);
  let graph_state = materialized.shared_kill_switch().state_handle();
  assert!(all_streams_share_kill_switch_state(&materializer, &graph_state));
}

#[test]
fn external_shared_kill_switch_shutdown_reaches_all_islands_after_split() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");
  let shared_kill_switch = SharedKillSwitch::new();
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .r#async()
    .into_mat(Sink::ignore(), KeepRight)
    .with_shared_kill_switch(&shared_kill_switch);
  let _materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  shared_kill_switch.shutdown();
  drive_registered_streams_until_all_terminal(&materializer);

  assert!(materializer.streams().iter().all(|stream| stream.state() == StreamState::Completed));
}

#[test]
fn materialized_unique_kill_switch_shutdown_completes_all_island_actors() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .r#async()
    .into_mat(Sink::ignore(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  materialized.unique_kill_switch().shutdown();

  wait_for_all_streams_to_reach_state(&materializer, StreamState::Completed);
}

#[test]
fn materialized_shared_kill_switch_shutdown_completes_all_island_actors() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .r#async()
    .into_mat(Sink::ignore(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  materialized.shared_kill_switch().shutdown();

  wait_for_all_streams_to_reach_state(&materializer, StreamState::Completed);
}

#[test]
fn materialized_unique_kill_switch_abort_sends_abort_to_all_island_actors() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .r#async()
    .into_mat(Sink::ignore(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  materialized.unique_kill_switch().abort(StreamError::failed_with_context("abort all islands"));

  wait_for_all_streams_to_reach_state(&materializer, StreamState::Failed);
}

#[test]
fn materialized_shared_kill_switch_abort_sends_abort_to_all_island_actors() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .r#async()
    .into_mat(Sink::ignore(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  materialized.shared_kill_switch().abort(StreamError::failed_with_context("abort all islands"));

  wait_for_all_streams_to_reach_state(&materializer, StreamState::Failed);
}

#[test]
fn downstream_cancel_sends_cancel_command_to_upstream_island_actor() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .r#async()
    .into_mat(Sink::head(), KeepRight);

  let materialized = graph.run(&mut materializer).expect("materialize");
  let deadline = Instant::now() + Duration::from_secs(5);
  while Instant::now() < deadline {
    if matches!(materialized.materialized().poll(), Completion::Ready(_)) {
      break;
    }
    thread::yield_now();
  }

  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(1_u32)));
  wait_for_upstream_cancel_command_count(&materializer, 1);
  assert_eq!(materializer.streams()[0].state(), StreamState::Cancelled);
}

#[test]
fn island_actor_diagnostics_does_not_propagate_downstream_cancellation() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .r#async()
    .into_mat(Sink::head(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  assert_eq!(materializer.streams()[0].drive(), DriveOutcome::Progressed);
  drive_registered_streams(&materializer, 8);
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(1_u32)));

  let diagnostics: Vec<StreamIslandActorDiagnostic> =
    materializer.island_actor_diagnostics_for_test().expect("island actor diagnostics should be available");
  let repeated: Vec<StreamIslandActorDiagnostic> =
    materializer.island_actor_diagnostics_for_test().expect("island actor diagnostics should be repeatable");
  let upstream_actor = diagnostics.first().expect("upstream island actor should be diagnosable");
  let repeated_upstream_actor = repeated.first().expect("upstream island actor should remain diagnosable");

  assert_eq!(upstream_actor.cancel_command_count(), 0);
  assert_eq!(repeated_upstream_actor.cancel_command_count(), 0);
}

#[test]
fn scheduled_drive_delivery_failure_fails_materialized_graph() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(
    system.clone(),
    ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)),
  );
  materializer.start().expect("start");
  let graph =
    Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic).r#async().into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");
  let diagnostics: Vec<StreamIslandActorDiagnostic> =
    materializer.island_actor_diagnostics_for_test().expect("island actor diagnostics should be available");
  let actor_pid = diagnostics.first().expect("island actor should be diagnosable").actor_pid();

  system.state().send_system_message(actor_pid, SystemMessage::Stop).expect("stop island actor");
  wait_for_actor_cell_removed(&system, actor_pid);

  wait_for_all_streams_to_reach_state(&materializer, StreamState::Failed);
}

#[test]
fn downstream_island_does_not_busy_loop_when_boundary_is_empty_and_open() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let graph =
    Source::<u32, _>::from_logic(StageKind::Custom, PendingSourceLogic).r#async().into_mat(Sink::head(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  for _ in 0..16 {
    assert_eq!(materializer.streams()[1].drive(), DriveOutcome::Idle);
  }

  assert_eq!(materialized.materialized().poll(), Completion::Pending);
  assert_eq!(materializer.streams()[1].state(), StreamState::Running);
}

#[test]
fn downstream_cancel_discards_in_flight_boundary_elements_after_head() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, EndlessSourceLogic::new())
    .r#async()
    .into_mat(Sink::head(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  assert_eq!(materializer.streams()[0].drive(), DriveOutcome::Progressed);
  drive_registered_streams(&materializer, 8);

  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(1_u32)));
  assert_eq!(materializer.streams()[0].state(), StreamState::Cancelled);
}

#[test]
fn detached_async_branch_does_not_hang_sibling_async_branch() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let side_sink = GraphDsl::create_sink(|builder: &mut GraphDslBuilder<u32, (), StreamNotUsed>| {
    let (async_inlet, async_outlet) =
      builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value).r#async()).expect("add async flow");
    let side_head_inlet = builder.add_sink(Sink::<u32, _>::head()).expect("add side head sink");
    builder.connect(&async_outlet, &side_head_inlet).expect("connect side async branch");
    let _ = async_inlet.id();
  });
  let graph =
    Source::from_array([1_u32, 2_u32]).also_to(side_sink).via(Flow::new().r#async()).into_mat(Sink::seq(), KeepRight);

  let materialized = graph.run(&mut materializer).expect("materialize");
  assert!(materializer.streams().len() >= 2);

  drive_registered_streams(&materializer, 16);

  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(vec![1_u32, 2_u32])));
  assert_eq!(materializer.streams().last().expect("main branch stream should exist").state(), StreamState::Completed);
}

#[test]
fn shutdown_drains_in_flight_boundary_elements_before_completing_graph() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let graph = Source::from_array([1_u32, 2_u32]).r#async().into_mat(Sink::seq(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  assert_eq!(materializer.streams()[0].drive(), DriveOutcome::Progressed);
  assert_eq!(materialized.materialized().poll(), Completion::Pending);

  materialized.unique_kill_switch().shutdown();
  drive_registered_streams(&materializer, 8);

  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(vec![1_u32, 2_u32])));
}

#[test]
fn abort_prioritizes_failure_over_in_flight_boundary_elements() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let graph = Source::from_array([1_u32, 2_u32]).r#async().into_mat(Sink::seq(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);
  let abort_error = StreamError::failed_with_context("abort in-flight boundary");

  assert_eq!(materializer.streams()[0].drive(), DriveOutcome::Progressed);
  assert_eq!(materialized.materialized().poll(), Completion::Pending);

  materialized.unique_kill_switch().abort(abort_error.clone());
  drive_registered_streams(&materializer, 8);

  assert_eq!(materialized.materialized().poll(), Completion::Ready(Err(abort_error)));
}

#[test]
fn abort_prioritizes_source_failure_when_both_are_pending() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let abort_error = StreamError::failed_with_context("graph abort");
  let source_failure = StreamError::failed_with_context("source failure");
  let graph = Source::<u32, _>::failed(source_failure).r#async().into_mat(Sink::seq(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  materialized.unique_kill_switch().abort(abort_error.clone());
  drive_registered_streams(&materializer, 8);

  assert_eq!(materialized.materialized().poll(), Completion::Ready(Err(abort_error)));
  assert!(materializer.streams().iter().all(|stream| stream.state() == StreamState::Failed));
}

#[test]
fn source_failure_prioritizes_shutdown_for_drainable_source() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_secs(60)));
  materializer.start().expect("start");
  let source_failure = StreamError::failed_with_context("source failure during shutdown");
  let graph =
    Source::<u32, _>::from_logic(StageKind::Custom, DrainOnShutdownFailingSourceLogic::new(source_failure.clone()))
      .r#async()
      .into_mat(Sink::seq(), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  assert_eq!(materializer.streams().len(), 2);

  materialized.unique_kill_switch().shutdown();
  drive_registered_streams(&materializer, 8);

  assert_eq!(materialized.materialized().poll(), Completion::Ready(Err(source_failure)));
}

#[test]
fn materialize_cancels_started_islands_when_later_island_start_fails() {
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  let sink: Sink<u32, StreamNotUsed> = Sink::from_logic(StageKind::SinkIgnore, FailOnStartSinkLogic);
  let graph = Source::single(1_u32).r#async().into_mat(sink, KeepRight);
  let result = graph.run(&mut materializer);

  assert!(matches!(result, Err(StreamError::Failed)));
  assert!(materializer.streams().is_empty());
  assert_eq!(materializer.snapshot().total_materialized(), 0);
}

#[test]
fn snapshot_reflects_stopped_state_after_shutdown() {
  // Given: a materializer that has been started, materialized, and shut down
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  materializer.shutdown().expect("shutdown");

  // When: taking a snapshot after shutdown
  let snap = materializer.snapshot();

  // Then: state is Stopped and count reflects the one materialization
  assert_eq!(snap.lifecycle_state(), MaterializerLifecycleState::Stopped);
  assert_eq!(snap.total_materialized(), 1);
}

// ---------------------------------------------------------------------------
// Diagnostics: new_without_system retains Idle state
// ---------------------------------------------------------------------------

#[test]
fn new_without_system_starts_idle() {
  // Given: a materializer without an actor system
  let materializer = ActorMaterializer::new_without_system(ActorMaterializerConfig::default());

  // Then: it is still Idle
  assert_eq!(materializer.lifecycle_state(), MaterializerLifecycleState::Idle);
  assert!(materializer.is_idle());
  assert_eq!(materializer.snapshot().total_materialized(), 0);
}
