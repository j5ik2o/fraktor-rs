use core::time::Duration;

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContext},
  error::ActorError,
  messaging::AnyMessageView,
  props::Props,
  scheduler::{
    SchedulerConfig,
    tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  system::{ActorSystem, ActorSystemConfig, remote::RemotingConfig},
};

use crate::core::{
  Completion, KeepRight, StreamError,
  lifecycle::StreamState,
  mat::{ActorMaterializer, ActorMaterializerConfig, MaterializerLifecycleState},
  stage::{Sink, Source},
};

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let config = ActorSystemConfig::default().with_scheduler_config(scheduler).with_tick_driver(tick_driver);
  ActorSystem::new_with_config(&props, &config).expect("system should build")
}

#[test]
fn start_fails_without_actor_system() {
  let mut materializer = ActorMaterializer::new_without_system(ActorMaterializerConfig::default());
  let result = materializer.start();
  assert!(matches!(result, Err(StreamError::ActorSystemMissing)));
}

#[test]
fn materialize_requires_start() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  let graph = Source::single(1_u32).to_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);
  assert!(matches!(result, Err(StreamError::MaterializerNotStarted)));
}

#[test]
fn actor_materializer_drives_stream() {
  let system = build_system();
  let controller = system.tick_driver_bundle().manual_controller().expect("manual controller").clone();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");
  let graph =
    Source::single(1_u32).map(|value| value + 1).to_mat(Sink::fold(0_u32, |acc, value| acc + value), KeepRight);
  let materialized = graph.run(&mut materializer).expect("materialize");
  for _ in 0..5 {
    controller.inject_and_drive(1);
  }
  assert_eq!(materialized.handle().state(), StreamState::Completed);
  assert_eq!(materialized.materialized().poll(), Completion::Ready(Ok(2)));
}

#[test]
fn shutdown_blocks_materialize() {
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
  materializer.shutdown().expect("shutdown");
  let graph = Source::single(1_u32).to_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);
  assert!(matches!(result, Err(StreamError::MaterializerStopped)));
}

#[test]
fn start_with_remoting_config() {
  let props = Props::from_fn(|| GuardianActor);
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let remoting = RemotingConfig::default().with_canonical_host("127.0.0.1").with_canonical_port(2552);
  let config = ActorSystemConfig::default()
    .with_scheduler_config(scheduler)
    .with_tick_driver(tick_driver)
    .with_remoting_config(remoting);
  let system = ActorSystem::new_with_config(&props, &config).expect("system should build");
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");
}

// ---------------------------------------------------------------------------
// Diagnostics: lifecycle_state()
// ---------------------------------------------------------------------------

#[test]
fn new_materializer_is_idle() {
  // Given: a freshly constructed materializer
  let system = build_system();
  let materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());

  // Then: lifecycle state is Idle
  assert_eq!(materializer.lifecycle_state(), MaterializerLifecycleState::Idle);
}

#[test]
fn started_materializer_is_running() {
  // Given: a materializer that has been started
  let system = build_system();
  let mut materializer = ActorMaterializer::new(system, ActorMaterializerConfig::default());
  materializer.start().expect("start");

  // Then: lifecycle state is Running
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
  let _controller = system.tick_driver_bundle().manual_controller().expect("manual controller").clone();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  // When: materializing a graph
  let graph = Source::single(1_u32).to_mat(Sink::head(), KeepRight);
  let _materialized = graph.run(&mut materializer).expect("materialize");

  // Then: total_materialized is 1
  assert_eq!(materializer.snapshot().total_materialized(), 1);

  // When: materializing a second graph
  let graph2 = Source::single(2_u32).to_mat(Sink::head(), KeepRight);
  let _materialized2 = graph2.run(&mut materializer).expect("materialize second");

  // Then: total_materialized is 2
  assert_eq!(materializer.snapshot().total_materialized(), 2);
}

#[test]
fn snapshot_reflects_stopped_state_after_shutdown() {
  // Given: a materializer that has been started, materialized, and shut down
  let system = build_system();
  let mut materializer =
    ActorMaterializer::new(system, ActorMaterializerConfig::default().with_drive_interval(Duration::from_millis(1)));
  materializer.start().expect("start");

  let graph = Source::single(1_u32).to_mat(Sink::head(), KeepRight);
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
