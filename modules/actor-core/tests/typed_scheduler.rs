use core::{
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};
use std::{thread::yield_now, time::Instant};

use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;
use fraktor_actor_core_rs::core::{
  kernel::actor::{actor_ref::ActorRef, scheduler::ExecutionBatch, setup::ActorSystemConfig},
  typed::{TypedActorRef, TypedActorSystem, TypedProps, dsl::Behaviors},
};
use fraktor_utils_core_rs::core::sync::ArcShared;

fn new_test_system() -> TypedActorSystem<u32> {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_start_time(Duration::from_secs(1));
  TypedActorSystem::<u32>::create_with_config(&guardian_props, config).expect("system")
}

fn null_receiver() -> TypedActorRef<u32> {
  TypedActorRef::<u32>::from_untyped(ActorRef::null())
}

fn noop_runnable(_batch: &ExecutionBatch) {}

#[test]
fn schedule_once_returns_handle() {
  let system = new_test_system();
  let scheduler = system.scheduler();

  let result = scheduler.schedule_once(Duration::from_millis(10), null_receiver(), 42_u32);

  let handle = result.expect("schedule_once should return a handle");
  assert!(!handle.is_cancelled(), "newly created handle should not be cancelled");

  system.terminate().expect("terminate");
}

#[test]
fn schedule_once_handle_is_cancellable() {
  let system = new_test_system();
  let scheduler = system.scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(100), null_receiver(), 1_u32).expect("handle");

  let cancelled = handle.cancel();

  assert!(cancelled, "cancel should succeed for a scheduled handle");
  assert!(handle.is_cancelled(), "handle should be cancelled after cancel()");

  system.terminate().expect("terminate");
}

#[test]
fn schedule_at_fixed_rate_returns_handle() {
  let system = new_test_system();
  let scheduler = system.scheduler();

  let result =
    scheduler.schedule_at_fixed_rate(Duration::from_millis(5), Duration::from_millis(10), null_receiver(), 7_u32);

  assert!(result.is_ok(), "schedule_at_fixed_rate should return a handle");

  system.terminate().expect("terminate");
}

#[test]
fn schedule_at_fixed_rate_handle_is_cancellable() {
  let system = new_test_system();
  let scheduler = system.scheduler();
  let handle = scheduler
    .schedule_at_fixed_rate(Duration::from_millis(5), Duration::from_millis(10), null_receiver(), 3_u32)
    .expect("handle");

  let cancelled = handle.cancel();

  assert!(cancelled, "cancel should succeed");
  assert!(handle.is_cancelled());

  system.terminate().expect("terminate");
}

#[test]
fn schedule_with_fixed_delay_returns_handle() {
  let system = new_test_system();
  let scheduler = system.scheduler();

  let result =
    scheduler.schedule_with_fixed_delay(Duration::from_millis(5), Duration::from_millis(20), null_receiver(), 99_u32);

  assert!(result.is_ok(), "schedule_with_fixed_delay should return a handle");

  system.terminate().expect("terminate");
}

#[test]
fn schedule_with_fixed_delay_handle_is_cancellable() {
  let system = new_test_system();
  let scheduler = system.scheduler();
  let handle = scheduler
    .schedule_with_fixed_delay(Duration::from_millis(5), Duration::from_millis(20), null_receiver(), 50_u32)
    .expect("handle");

  let cancelled = handle.cancel();

  assert!(cancelled, "cancel should succeed");
  assert!(handle.is_cancelled());

  system.terminate().expect("terminate");
}

#[test]
fn scheduler_is_cloneable() {
  let system = new_test_system();
  let scheduler = system.scheduler();

  let cloned = scheduler.clone();

  let h1 = scheduler.schedule_once(Duration::from_millis(10), null_receiver(), 1_u32);
  let h2 = cloned.schedule_once(Duration::from_millis(10), null_receiver(), 2_u32);
  assert!(h1.is_ok());
  assert!(h2.is_ok());

  system.terminate().expect("terminate");
}

#[test]
fn schedule_once_runnable_returns_handle() {
  let system = new_test_system();
  let scheduler = system.scheduler();

  let result = scheduler.schedule_once_runnable(Duration::from_millis(10), noop_runnable);

  assert!(result.is_ok(), "schedule_once_runnable should return a handle");

  system.terminate().expect("terminate");
}

#[test]
fn schedule_once_runnable_executes_when_context_runs() {
  let system = new_test_system();
  let scheduler = system.scheduler();
  let executions = ArcShared::new(AtomicUsize::new(0));

  let handle = scheduler
    .schedule_once_runnable(Duration::from_millis(1), {
      let executions = executions.clone();
      move |_batch: &ExecutionBatch| {
        executions.fetch_add(1, Ordering::Relaxed);
      }
    })
    .expect("schedule_once_runnable should return a handle");
  assert!(!handle.is_cancelled());

  let deadline = Instant::now() + Duration::from_secs(1);
  while Instant::now() < deadline && executions.load(Ordering::Relaxed) == 0 {
    yield_now();
  }

  assert_eq!(executions.load(Ordering::Relaxed), 1);

  system.terminate().expect("terminate");
}

#[test]
fn schedule_at_fixed_rate_runnable_returns_handle() {
  let system = new_test_system();
  let scheduler = system.scheduler();

  let result =
    scheduler.schedule_at_fixed_rate_runnable(Duration::from_millis(5), Duration::from_millis(10), noop_runnable);

  assert!(result.is_ok(), "schedule_at_fixed_rate_runnable should return a handle");

  system.terminate().expect("terminate");
}

#[test]
fn schedule_with_fixed_delay_runnable_returns_handle() {
  let system = new_test_system();
  let scheduler = system.scheduler();

  let result =
    scheduler.schedule_with_fixed_delay_runnable(Duration::from_millis(5), Duration::from_millis(20), noop_runnable);

  assert!(result.is_ok(), "schedule_with_fixed_delay_runnable should return a handle");

  system.terminate().expect("terminate");
}
