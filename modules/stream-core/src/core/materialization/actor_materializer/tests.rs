extern crate std;

use core::time::Duration;
use std::time::Instant;

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext, error::ActorError, messaging::AnyMessageView, props::Props, scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::{ActorSystem, remote::RemotingConfig},
};

use crate::core::{
  StreamError,
  dsl::{Sink, Source},
  r#impl::materialization::StreamState,
  materialization::{ActorMaterializer, ActorMaterializerConfig, Completion, KeepRight, MaterializerLifecycleState},
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
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_scheduler_config(scheduler);
  ActorSystem::create_with_config(&props, config).expect("system should build")
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
  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let result = graph.run(&mut materializer);
  assert!(matches!(result, Err(StreamError::MaterializerNotStarted)));
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
