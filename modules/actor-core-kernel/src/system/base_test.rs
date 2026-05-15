use alloc::{boxed::Box, format, vec, vec::Vec};
use core::{
  pin::Pin,
  sync::atomic::{AtomicBool, AtomicUsize, Ordering},
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  time::Duration,
};

use fraktor_utils_core_rs::{
  collections::queue::capabilities::{QueueCapabilityRegistry, QueueCapabilitySet},
  sync::{ArcShared, SharedAccess, SpinSyncMutex},
  timing::delay::{DelayFuture, DelayProvider},
};

use super::ActorSystem;
use crate::{
  actor::{
    Actor, ActorCell, ActorContext, Address, Pid,
    actor_path::{ActorPath, ActorPathParts, ActorPathScheme},
    actor_ref::{ActorRef, NullSender},
    actor_ref_provider::{ActorRefProvider, ActorRefProviderHandleShared, ActorRefResolveError},
    deploy::{Deploy, Deployer, RemoteScope, Scope},
    error::{ActorError, ActorErrorReason, SendError},
    lifecycle::LifecycleStage,
    messaging::{AnyMessage, AnyMessageView, system_message::SystemMessage},
    props::{DeployablePropsMetadata, MailboxConfig, MailboxRequirement, Props},
    scheduler::{
      SchedulerConfig,
      task_run::{TaskRunError, TaskRunPriority},
      tick_driver::{
        AutoDriverMetadata, AutoProfileKind, SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverId,
        TickDriverKind, TickDriverProvision, TickDriverStopper, TickFeedHandle, next_tick_driver_id,
        tests::TestTickDriver,
      },
    },
    setup::ActorSystemConfig,
    spawn::SpawnError,
  },
  dispatch::dispatcher::{
    DefaultDispatcherFactory, DispatcherConfig, ExecuteError, Executor, ExecutorShared, MessageDispatcherFactory,
    TrampolineState,
  },
  event::stream::{EventStreamEvent, EventStreamSubscriber, tests::subscriber_handle},
  system::{
    TerminationSignal,
    base::LogLevel,
    remote::{RemoteDeploymentHook, RemoteDeploymentOutcome, RemoteDeploymentRequest, RemotingConfig},
    state::{SystemStateShared, system_state::SystemState},
  },
};

impl ActorSystem {
  /// Creates a bootstrapped actor system with a no-op user guardian.
  ///
  /// Inline-test only helper. External callers should use `create_noop_actor_system` from
  /// `fraktor-actor-adaptor-std-rs`.
  ///
  /// # Panics
  ///
  /// Panics if the default test-support configuration fails to build.
  #[must_use]
  pub(crate) fn new_empty() -> Self {
    Self::new_empty_with(|config| config)
  }

  /// Creates a bootstrapped actor system with a customizable config.
  ///
  /// See [`Self::new_empty`] for the rationale on the cfg gating.
  ///
  /// # Panics
  ///
  /// Panics if the default test-support configuration fails to build.
  #[must_use]
  pub(crate) fn new_empty_with<F>(configure: F) -> Self
  where
    F: FnOnce(ActorSystemConfig) -> ActorSystemConfig, {
    use crate::actor::scheduler::tick_driver::tests::TestTickDriver;

    let config = configure(ActorSystemConfig::new(TestTickDriver::default()));
    Self::create_with_noop_guardian(config).expect("test-support config should build in new_empty_with")
  }

  /// Creates an unbootstrapped actor system without any guardian.
  ///
  /// Use only for tests that explicitly exercise pre-bootstrap behavior.
  #[must_use]
  pub(crate) fn new_unbootstrapped() -> Self {
    Self::new_unbootstrapped_with(|config| config)
  }

  /// Creates an unbootstrapped actor system with a customizable config.
  ///
  /// Use only for tests that explicitly exercise pre-bootstrap behavior.
  #[must_use]
  pub(crate) fn new_unbootstrapped_with<F>(configure: F) -> Self
  where
    F: FnOnce(ActorSystemConfig) -> ActorSystemConfig, {
    use crate::actor::scheduler::tick_driver::tests::TestTickDriver;

    let config = configure(ActorSystemConfig::new(TestTickDriver::default()));
    let state = SystemState::build_from_owned_config(config)
      .expect("test-support config should build in new_unbootstrapped_with");
    Self::from_system_state(SystemStateShared::new(state))
  }
}

struct TestActor;

impl Actor for TestActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Clone)]
enum TestRemoteDeploymentOutcome {
  RemoteCreated(ActorRef),
  UseLocalDeployment,
  Failed(&'static str),
}

struct RecordingRemoteDeploymentHook {
  calls:   ArcShared<SpinSyncMutex<Vec<RemoteDeploymentRequest>>>,
  outcome: TestRemoteDeploymentOutcome,
}

impl RecordingRemoteDeploymentHook {
  fn new(calls: ArcShared<SpinSyncMutex<Vec<RemoteDeploymentRequest>>>, outcome: TestRemoteDeploymentOutcome) -> Self {
    Self { calls, outcome }
  }
}

impl RemoteDeploymentHook for RecordingRemoteDeploymentHook {
  fn deploy_child(&self, request: RemoteDeploymentRequest) -> RemoteDeploymentOutcome {
    self.calls.lock().push(request);
    match self.outcome.clone() {
      | TestRemoteDeploymentOutcome::RemoteCreated(actor) => RemoteDeploymentOutcome::RemoteCreated(actor),
      | TestRemoteDeploymentOutcome::UseLocalDeployment => RemoteDeploymentOutcome::UseLocalDeployment,
      | TestRemoteDeploymentOutcome::Failed(reason) => RemoteDeploymentOutcome::Failed(reason.into()),
    }
  }
}

fn remote_deployer_for_child(name: &str) -> Deployer {
  let mut deployer = Deployer::new();
  let remote = Address::remote("remote-system", "remote.example.com", 2552);
  deployer.register(format!("/user/{name}"), Deploy::new().with_scope(Scope::Remote(RemoteScope::new(remote))));
  deployer
}

fn deployable_test_props() -> Props {
  Props::from_fn(|| TestActor)
    .with_deployable_metadata(DeployablePropsMetadata::new("echo", AnyMessage::new(String::from("payload"))))
}

fn remote_created_ref(name: &str) -> ActorRef {
  let parts = ActorPathParts::with_authority("remote-system", Some(("remote.example.com", 2552)));
  let path = ActorPath::from_parts(parts).child(name);
  ActorRef::with_canonical_path(Pid::new(900, 0), NullSender, path)
}

fn assert_remote_lifecycle_unsupported(result: Result<(), SendError>) {
  match result {
    | Err(SendError::InvalidPayload { context, .. }) => {
      assert_eq!(context, "remote child lifecycle command is not supported");
    },
    | other => panic!("expected unsupported remote lifecycle error, got {other:?}"),
  }
}

#[test]
fn remote_deployment_spawn_invokes_hook_without_local_cell() {
  let calls: ArcShared<SpinSyncMutex<Vec<RemoteDeploymentRequest>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let system = ActorSystem::new_empty_with(|config| config.with_deployer(remote_deployer_for_child("remote-child")));
  system.extended().register_remote_deployment_hook(RecordingRemoteDeploymentHook::new(
    calls.clone(),
    TestRemoteDeploymentOutcome::RemoteCreated(remote_created_ref("remote-child")),
  ));

  let child = system.actor_of_named(&deployable_test_props(), "remote-child").expect("remote child should spawn");
  let requests = calls.lock().clone();
  let request = requests.first().expect("hook should be invoked");
  let metadata = request.deployable_metadata().expect("deployable metadata");

  assert_eq!(requests.len(), 1);
  assert_eq!(request.child_pid(), Pid::new(0, 0));
  assert_eq!(request.child_name(), "remote-child");
  assert_eq!(request.child_path().to_relative_string(), "/user/remote-child");
  assert_eq!(request.scope().node(), &Address::remote("remote-system", "remote.example.com", 2552));
  assert_eq!(metadata.factory_id(), "echo");
  assert_eq!(metadata.payload().downcast_ref::<String>().map(String::as_str), Some("payload"));
  assert_eq!(child.pid(), Pid::new(900, 0));
  assert!(system.actor_ref_by_pid(child.pid()).is_none());
}

#[test]
fn remote_deployment_success_releases_local_name_reservation() {
  let calls: ArcShared<SpinSyncMutex<Vec<RemoteDeploymentRequest>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let system = ActorSystem::new_empty_with(|config| config.with_deployer(remote_deployer_for_child("remote-child")));
  system.extended().register_remote_deployment_hook(RecordingRemoteDeploymentHook::new(
    calls.clone(),
    TestRemoteDeploymentOutcome::RemoteCreated(remote_created_ref("remote-child")),
  ));

  let first = system.actor_of_named(&deployable_test_props(), "remote-child").expect("first remote child should spawn");
  let second = system
    .actor_of_named(&deployable_test_props(), "remote-child")
    .expect("name should be reusable after remote spawn");

  assert_eq!(first.pid(), Pid::new(900, 0));
  assert_eq!(second.pid(), Pid::new(900, 0));
  assert_eq!(calls.lock().len(), 2);
}

#[test]
fn remote_deployment_use_local_outcome_continues_local_spawn() {
  let calls: ArcShared<SpinSyncMutex<Vec<RemoteDeploymentRequest>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let system = ActorSystem::new_empty_with(|config| config.with_deployer(remote_deployer_for_child("loopback-child")));
  system.extended().register_remote_deployment_hook(RecordingRemoteDeploymentHook::new(
    calls.clone(),
    TestRemoteDeploymentOutcome::UseLocalDeployment,
  ));

  let child = system.actor_of_named(&deployable_test_props(), "loopback-child").expect("local child should spawn");

  assert_eq!(calls.lock().len(), 1);
  assert!(system.actor_ref_by_pid(child.pid()).is_some());
}

#[test]
fn remote_deployment_failed_outcome_does_not_fallback_to_local_spawn() {
  let calls: ArcShared<SpinSyncMutex<Vec<RemoteDeploymentRequest>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let system = ActorSystem::new_empty_with(|config| config.with_deployer(remote_deployer_for_child("failed-child")));
  system.extended().register_remote_deployment_hook(RecordingRemoteDeploymentHook::new(
    calls.clone(),
    TestRemoteDeploymentOutcome::Failed("remote create failed"),
  ));

  let result = system.actor_of_named(&deployable_test_props(), "failed-child");

  match result {
    | Err(SpawnError::InvalidProps(reason)) => assert_eq!(reason, "remote create failed"),
    | other => panic!("expected invalid props, got {other:?}"),
  }
  let request = calls.lock().first().cloned().expect("hook request");
  assert!(system.actor_ref_by_pid(request.child_pid()).is_none());
}

#[test]
fn remote_deployment_missing_hook_is_spawn_error() {
  let system =
    ActorSystem::new_empty_with(|config| config.with_deployer(remote_deployer_for_child("missing-hook-child")));

  let result = system.actor_of_named(&deployable_test_props(), "missing-hook-child");

  match result {
    | Err(SpawnError::InvalidProps(reason)) => assert_eq!(reason, "remote deployment hook is not installed"),
    | other => panic!("expected invalid props, got {other:?}"),
  }
}

#[test]
fn remote_deployment_non_deployable_remote_create_is_spawn_error() {
  let calls: ArcShared<SpinSyncMutex<Vec<RemoteDeploymentRequest>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let system =
    ActorSystem::new_empty_with(|config| config.with_deployer(remote_deployer_for_child("non-deployable-child")));
  system.extended().register_remote_deployment_hook(RecordingRemoteDeploymentHook::new(
    calls.clone(),
    TestRemoteDeploymentOutcome::RemoteCreated(remote_created_ref("non-deployable-child")),
  ));

  let result = system.actor_of_named(&Props::from_fn(|| TestActor), "non-deployable-child");

  match result {
    | Err(SpawnError::InvalidProps(reason)) => {
      assert_eq!(reason, "remote deployment requires deployable props metadata");
    },
    | other => panic!("expected invalid props, got {other:?}"),
  }
  assert!(calls.lock().is_empty());
}

#[test]
fn remote_deployment_timeout_failure_is_spawn_error() {
  let system = ActorSystem::new_empty_with(|config| config.with_deployer(remote_deployer_for_child("timeout-child")));
  system.extended().register_remote_deployment_hook(RecordingRemoteDeploymentHook::new(
    ArcShared::new(SpinSyncMutex::new(Vec::new())),
    TestRemoteDeploymentOutcome::Failed("remote deployment timed out"),
  ));

  let result = system.actor_of_named(&deployable_test_props(), "timeout-child");

  match result {
    | Err(SpawnError::InvalidProps(reason)) => assert_eq!(reason, "remote deployment timed out"),
    | other => panic!("expected invalid props, got {other:?}"),
  }
}

#[test]
fn remote_deployment_remote_child_lifecycle_commands_are_unsupported() {
  let calls: ArcShared<SpinSyncMutex<Vec<RemoteDeploymentRequest>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let system = ActorSystem::new_empty_with(|config| config.with_deployer(remote_deployer_for_child("lifecycle-child")));
  system.extended().register_remote_deployment_hook(RecordingRemoteDeploymentHook::new(
    calls,
    TestRemoteDeploymentOutcome::RemoteCreated(remote_created_ref("lifecycle-child")),
  ));
  let child = system.actor_of_named(&deployable_test_props(), "lifecycle-child").expect("remote child should spawn");

  assert_remote_lifecycle_unsupported(child.stop());
  assert_remote_lifecycle_unsupported(child.suspend());
  assert_remote_lifecycle_unsupported(child.resume());
}

#[test]
fn terminated_local_child_with_canonical_authority_is_not_remote_child() {
  let remoting = RemotingConfig::default().with_canonical_host("127.0.0.1").with_canonical_port(2552);
  let system = ActorSystem::new_empty_with(|config| config.with_remoting_config(remoting));
  let child = system.actor_of_named(&Props::from_fn(|| TestActor), "local-child").expect("local child should spawn");
  let canonical_path = child.actor_ref().canonical_path().expect("canonical path");

  assert!(canonical_path.parts().authority_endpoint().is_some());

  system.state().remove_cell(&child.pid());
  let result = child.stop();

  assert!(matches!(result, Err(SendError::Closed(_))));
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
  let system = ActorSystem::new_unbootstrapped();
  let props = Props::from_fn(|| TestActor);
  let err = system.spawn_child(Pid::new(999, 0), &props).unwrap_err();
  assert!(matches!(err, SpawnError::SystemNotBootstrapped));
}

#[test]
fn resolve_actor_ref_fails_before_root_started() {
  let system = ActorSystem::new_unbootstrapped();
  let path = ActorPath::root();
  let err = system.resolve_actor_ref(path).unwrap_err();
  assert!(matches!(err, ActorRefResolveError::SystemNotBootstrapped));
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
  let settings = DispatcherConfig::with_defaults("noop");
  let executor = ExecutorShared::new(Box::new(NoopExecutor), TrampolineState::new());
  let configurator: Box<dyn MessageDispatcherFactory> = Box::new(DefaultDispatcherFactory::new(&settings, executor));
  ArcShared::new(configurator)
}

/// Noop stopper used in `StaticTickDriver`.
struct NoopStopper;

impl TickDriverStopper for NoopStopper {
  fn stop(self: Box<Self>) {}
}

/// Static tick driver that provisions immediately without starting any background threads.
struct StaticTickDriver {
  id:         TickDriverId,
  kind:       TickDriverKind,
  resolution: Duration,
  metadata:   Option<AutoDriverMetadata>,
}

impl StaticTickDriver {
  const fn new(id: TickDriverId, kind: TickDriverKind, resolution: Duration) -> Self {
    Self { id, kind, resolution, metadata: None }
  }

  fn with_auto_metadata(mut self, metadata: AutoDriverMetadata) -> Self {
    self.metadata = Some(metadata);
    self
  }
}

impl TickDriver for StaticTickDriver {
  fn kind(&self) -> TickDriverKind {
    self.kind
  }

  fn provision(
    self: Box<Self>,
    _feed: TickFeedHandle,
    _executor: SchedulerTickExecutor,
  ) -> Result<TickDriverProvision, TickDriverError> {
    Ok(TickDriverProvision {
      resolution:    self.resolution,
      id:            self.id,
      kind:          self.kind,
      stopper:       Box::new(NoopStopper),
      auto_metadata: self.metadata,
    })
  }
}

/// Tick driver that records when its stopper is called.
struct ShutdownRecordingDriver {
  resolution:     Duration,
  shutdown_calls: ArcShared<AtomicUsize>,
}

impl ShutdownRecordingDriver {
  fn new(resolution: Duration, shutdown_calls: ArcShared<AtomicUsize>) -> Self {
    Self { resolution, shutdown_calls }
  }
}

impl TickDriver for ShutdownRecordingDriver {
  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn provision(
    self: Box<Self>,
    _feed: TickFeedHandle,
    _executor: SchedulerTickExecutor,
  ) -> Result<TickDriverProvision, TickDriverError> {
    Ok(TickDriverProvision {
      resolution:    self.resolution,
      id:            next_tick_driver_id(),
      kind:          TickDriverKind::Auto,
      stopper:       Box::new(ShutdownRecordingStopper {
        shutdown_calls: self.shutdown_calls,
        did_shutdown:   AtomicBool::new(false),
      }),
      auto_metadata: None,
    })
  }
}

struct ShutdownRecordingStopper {
  shutdown_calls: ArcShared<AtomicUsize>,
  did_shutdown:   AtomicBool,
}

impl TickDriverStopper for ShutdownRecordingStopper {
  fn stop(self: Box<Self>) {
    if !self.did_shutdown.swap(true, Ordering::SeqCst) {
      self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
    }
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
  assert_eq!(snapshot.kind, TickDriverKind::Manual);
  assert!(system.scheduler().with_read(|s| s.config().runner_api_enabled()));
}

#[test]
fn actor_system_drop_shuts_down_executor_once() {
  let executor_calls = ArcShared::new(AtomicUsize::new(0));
  let tick_driver = ShutdownRecordingDriver::new(Duration::from_millis(1), executor_calls.clone());
  let config = ActorSystemConfig::new(tick_driver);

  let state = SystemState::build_from_owned_config(config).expect("state");
  let system = ActorSystem::from_system_state(SystemStateShared::new(state));
  drop(system);

  assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn actor_system_new_with_config_and_allows_extra_top_level_registration_in_configure() {
  let props = Props::from_fn(|| TestActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);

  let system = ActorSystem::create_from_props_with_init(&props, config, |system| {
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
  assert!(matches!(late, Err(crate::system::RegisterExtraTopLevelError::AlreadyStarted)));
}

#[test]
fn actor_system_create_from_props_with_init_can_spawn_system_top_level_actor() {
  let props = Props::from_fn(|| TestActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);

  let system = ActorSystem::create_from_props_with_init(&props, config, |system| {
    let top_level_props = Props::from_fn(|| TestActor).with_name("metrics");
    system.extended().spawn_system_top_level_actor(&top_level_props, "metrics")?;
    Ok(())
  })
  .expect("system should build");

  assert!(system.state().extra_top_level("metrics").is_some());
}

#[test]
fn bootstrap_rolls_back_system_top_level_actor_when_registration_fails() {
  let props = Props::from_fn(|| TestActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let state = SystemStateShared::new(SystemState::build_from_owned_config(config).expect("state"));
  state.register_extra_top_level("metrics", ActorRef::null()).expect("pre-register metrics");
  let system = ActorSystem::from_system_state(state);

  let result = system.bootstrap(&props, |system| {
    let top_level_props = Props::from_fn(|| TestActor).with_name("metrics");
    system.extended().spawn_system_top_level_actor(&top_level_props, "metrics")?;
    Ok(())
  });

  match result {
    | Err(SpawnError::SystemBuildError(message)) => {
      assert!(message.contains("system top-level registration failed"));
      assert!(message.contains("DuplicateName"));
    },
    | other => panic!("unexpected bootstrap result: {other:?}"),
  }

  let system_guardian_pid = system.state().system_guardian_pid().expect("system guardian pid");
  assert!(system.children(system_guardian_pid).is_empty());
  assert!(system.state().extra_top_level("metrics").is_some());
}

#[test]
fn actor_system_create_from_props_with_init_fails_without_tick_driver() {
  let props = Props::from_fn(|| TestActor);
  let config = ActorSystemConfig::default();
  match ActorSystem::create_from_props_with_init(&props, config, |_| Ok(())) {
    | Ok(_) => panic!("system should not build without tick driver"),
    | Err(SpawnError::SystemBuildError(message)) => assert!(message.contains("tick driver is required")),
    | Err(other) => panic!("unexpected error: {other:?}"),
  };
}

#[test]
fn actor_system_create_with_noop_guardian_bootstraps_user_guardian() {
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("noop-system");

  let system = ActorSystem::create_with_noop_guardian(config).expect("system should build");

  assert!(system.state().has_root_started());
  let user_guardian = system.user_guardian_ref();
  let path = user_guardian.path().expect("user guardian path");
  assert_eq!(path.to_relative_string(), "/user");
}

#[test]
fn actor_system_create_with_noop_guardian_spawns_user_child() {
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("noop-child-system");
  let system = ActorSystem::create_with_noop_guardian(config).expect("system should build");

  let child = system.actor_of(&Props::from_fn(|| TestActor)).expect("spawn child");
  let path = child.actor_ref().path().expect("child path");

  assert!(path.to_relative_string().starts_with("/user/"));
}

#[test]
fn actor_system_create_with_noop_guardian_fails_without_tick_driver() {
  let result = ActorSystem::create_with_noop_guardian(ActorSystemConfig::default());

  match result {
    | Err(SpawnError::SystemBuildError(message)) => assert!(message.contains("tick driver is required")),
    | Ok(_) => panic!("system should not build without tick driver"),
    | Err(other) => panic!("unexpected error: {other:?}"),
  }
}

#[test]
fn actor_system_from_system_state() {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  let state = SystemState::build_from_owned_config(config).expect("state");
  let system = ActorSystem::from_system_state(SystemStateShared::new(state));
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
  let tick_driver = StaticTickDriver::new(driver_id, TickDriverKind::Auto, resolution)
    .with_auto_metadata(AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id, resolution });
  let config = ActorSystemConfig::new(tick_driver);
  let state = SystemState::build_from_owned_config(config).expect("state");
  let system = ActorSystem::from_system_state(SystemStateShared::new(state));

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
  use fraktor_utils_core_rs::collections::queue::capabilities::{QueueCapabilityRegistry, QueueCapabilitySet};

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
  let system = ActorSystem::new_unbootstrapped();
  let props = Props::from_fn(|| TestActor);

  let result = system.spawn(&props);
  assert!(result.is_err());
}

fn make_test_system() -> ActorSystem {
  make_test_system_with_name("test-system")
}

fn make_test_system_with_name(name: &str) -> ActorSystem {
  let props = Props::from_fn(|| TestActor);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name(name);
  ActorSystem::create_from_props(&props, config).expect("system")
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
    ActorSystem::new_empty_with(|config| config.with_dispatcher_factory("noop", noop_dispatcher_configurator()));
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
    ActorSystem::new_empty_with(|config| config.with_dispatcher_factory("noop", noop_dispatcher_configurator()));

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
    ActorSystem::new_empty_with(|config| config.with_dispatcher_factory("noop", noop_dispatcher_configurator()));
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
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");
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
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("system");
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

impl crate::actor::scheduler::task_run::TaskRunOnClose for RecordingShutdownTask {
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
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(TestTickDriver::default())).expect("actor system");
  let mut provider = system.delay_provider();
  let mut future = provider.delay(Duration::from_millis(1));
  assert!(matches!(poll_delay_future(&mut future), Poll::Pending));

  system.scheduler().with_write(|s| s.run_for_test(1));

  assert!(matches!(poll_delay_future(&mut future), Poll::Ready(())));
}

#[test]
fn lifecycle_events_cover_restart_transitions() {
  let system = ActorSystem::new_unbootstrapped();
  let stages: ArcShared<SpinSyncMutex<Vec<LifecycleStage>>> = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(LifecycleEventWatcher::new(stages.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  let props = Props::from_fn(|| TestActor);
  let child = system.spawn_with_parent(None, &props).expect("spawn succeeds");
  let pid = child.pid();

  // AC-H4: `handle_recreate` の `start_recreate` 段は Pekko `faultRecreate`
  // 契約に従い「呼び出し時点で mailbox が suspended である」ことを前提とする
  // （先行する `report_failure` が suspend する経路が production path）。
  // この統合テストでは failure を経由しないため、手動で mailbox を suspend
  // してから Recreate を送る。
  let cell = system.state().cell(&pid).expect("cell registered");
  cell.mailbox().suspend();

  system
    .state()
    .send_system_message(pid, SystemMessage::Recreate(ActorErrorReason::new("lifecycle-restart-test")))
    .expect("recreate enqueued");

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
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_remoting_config(remoting);
  let state = SystemState::build_from_owned_config(config).expect("state");
  let system = ActorSystem::from_system_state(SystemStateShared::new(state));

  let recorded = ArcShared::new(SpinSyncMutex::new(None));
  let actor_ref_provider_handle_shared =
    ActorRefProviderHandleShared::new(DummyActorRefProvider::new(recorded.clone()));
  system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared).expect("register provider");
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
fn spawn_child_at_rejects_resolved_non_local_parent() {
  let remoting = RemotingConfig::default().with_canonical_host("example.com").with_canonical_port(2552);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_remoting_config(remoting);
  let state = SystemState::build_from_owned_config(config).expect("state");
  let system = ActorSystem::from_system_state(SystemStateShared::new(state));
  let actor_ref_provider_handle_shared =
    ActorRefProviderHandleShared::new(DummyActorRefProvider::new(ArcShared::new(SpinSyncMutex::new(None))));
  system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared).expect("register provider");
  system.state().mark_root_started();

  let result = system.spawn_child_at(ActorPath::root().child("remote-parent"), &Props::from_fn(|| TestActor), "child");

  match result {
    | Err(SpawnError::InvalidProps(reason)) => assert_eq!(reason, "target parent path is not a local actor"),
    | other => panic!("expected invalid props, got {other:?}"),
  }
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
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_remoting_config(remoting);
  let state = SystemState::build_from_owned_config(config).expect("state");
  let system = ActorSystem::from_system_state(SystemStateShared::new(state));
  system.state().mark_root_started();

  let path = ActorPath::root().child("svc");
  let result = system.resolve_actor_ref(path);

  assert!(matches!(result, Err(ActorRefResolveError::ProviderMissing)));
}

#[test]
fn guardian_refs_preserve_canonical_authority() {
  let user_props = Props::from_fn(|| TestActor).with_name("user-guardian");
  let remoting = RemotingConfig::default().with_canonical_host("guardian.example.com").with_canonical_port(4101);
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_system_name("guardian-compat")
    .with_remoting_config(remoting);

  let system = ActorSystem::create_from_props(&user_props, config).expect("actor system bootstrap");

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
