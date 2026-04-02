use core::time::Duration;

use crate::core::{
  kernel::actor::{
    actor_ref::ActorRef,
    scheduler::{ExecutionBatch, SchedulerConfig, SchedulerContext},
  },
  typed::{TypedActorRef, internal::TypedSchedulerShared, scheduler::Scheduler},
};

// --- helpers ---------------------------------------------------------------

fn new_scheduler() -> Scheduler {
  let context = SchedulerContext::new(SchedulerConfig::default());
  let shared = TypedSchedulerShared::new(context.scheduler());
  Scheduler::new(shared)
}

fn null_receiver() -> TypedActorRef<u32> {
  TypedActorRef::<u32>::from_untyped(ActorRef::null())
}

fn noop_runnable(_batch: &ExecutionBatch) {}

// --- schedule_once ---------------------------------------------------------

#[test]
fn schedule_once_returns_handle() {
  // Given: a Scheduler facade
  let scheduler = new_scheduler();

  // When: schedule_once is called with a valid delay
  let result = scheduler.schedule_once(Duration::from_millis(10), null_receiver(), 42u32);

  // Then: a SchedulerHandle is returned
  let handle = result.expect("schedule_once should return a handle");
  assert!(!handle.is_cancelled(), "newly created handle should not be cancelled");
}

#[test]
fn schedule_once_handle_is_cancellable() {
  // Given: a scheduled-once job
  let scheduler = new_scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(100), null_receiver(), 1u32).expect("handle");

  // When: cancel is called
  let cancelled = handle.cancel();

  // Then: the handle reports cancellation
  assert!(cancelled, "cancel should succeed for a scheduled handle");
  assert!(handle.is_cancelled(), "handle should be cancelled after cancel()");
}

// --- schedule_at_fixed_rate ------------------------------------------------

#[test]
fn schedule_at_fixed_rate_returns_handle() {
  // Given: a Scheduler facade
  let scheduler = new_scheduler();

  // When: schedule_at_fixed_rate is called
  let result =
    scheduler.schedule_at_fixed_rate(Duration::from_millis(5), Duration::from_millis(10), null_receiver(), 7u32);

  // Then: a SchedulerHandle is returned
  assert!(result.is_ok(), "schedule_at_fixed_rate should return a handle");
}

#[test]
fn schedule_at_fixed_rate_handle_is_cancellable() {
  // Given: a fixed-rate scheduled job
  let scheduler = new_scheduler();
  let handle = scheduler
    .schedule_at_fixed_rate(Duration::from_millis(5), Duration::from_millis(10), null_receiver(), 3u32)
    .expect("handle");

  // When: cancel is called
  let cancelled = handle.cancel();

  // Then: the handle is cancelled
  assert!(cancelled, "cancel should succeed");
  assert!(handle.is_cancelled());
}

// --- schedule_with_fixed_delay ---------------------------------------------

#[test]
fn schedule_with_fixed_delay_returns_handle() {
  // Given: a Scheduler facade
  let scheduler = new_scheduler();

  // When: schedule_with_fixed_delay is called
  let result =
    scheduler.schedule_with_fixed_delay(Duration::from_millis(5), Duration::from_millis(20), null_receiver(), 99u32);

  // Then: a SchedulerHandle is returned
  assert!(result.is_ok(), "schedule_with_fixed_delay should return a handle");
}

#[test]
fn schedule_with_fixed_delay_handle_is_cancellable() {
  // Given: a fixed-delay scheduled job
  let scheduler = new_scheduler();
  let handle = scheduler
    .schedule_with_fixed_delay(Duration::from_millis(5), Duration::from_millis(20), null_receiver(), 50u32)
    .expect("handle");

  // When: cancel is called
  let cancelled = handle.cancel();

  // Then: the handle is cancelled
  assert!(cancelled, "cancel should succeed");
  assert!(handle.is_cancelled());
}

// --- Clone -----------------------------------------------------------------

#[test]
fn scheduler_is_cloneable() {
  // Given: a Scheduler facade
  let scheduler = new_scheduler();

  // When: the scheduler is cloned
  let cloned = scheduler.clone();

  // Then: both instances can schedule independently
  let h1 = scheduler.schedule_once(Duration::from_millis(10), null_receiver(), 1u32);
  let h2 = cloned.schedule_once(Duration::from_millis(10), null_receiver(), 2u32);
  assert!(h1.is_ok());
  assert!(h2.is_ok());
}

// --- runnable scheduling surface -------------------------------------------

#[test]
fn schedule_once_runnable_returns_handle() {
  // Given: a Scheduler facade
  let scheduler = new_scheduler();

  // When: schedule_once_runnable is called with a closure-style runnable
  let result = scheduler.schedule_once_runnable(Duration::from_millis(10), noop_runnable);

  // Then: a SchedulerHandle is returned
  assert!(result.is_ok(), "schedule_once_runnable should return a handle");
}

#[test]
fn schedule_at_fixed_rate_runnable_returns_handle() {
  // Given: a Scheduler facade
  let scheduler = new_scheduler();

  // When: schedule_at_fixed_rate_runnable is called with a closure-style runnable
  let result =
    scheduler.schedule_at_fixed_rate_runnable(Duration::from_millis(5), Duration::from_millis(10), noop_runnable);

  // Then: a SchedulerHandle is returned
  assert!(result.is_ok(), "schedule_at_fixed_rate_runnable should return a handle");
}

#[test]
fn schedule_with_fixed_delay_runnable_returns_handle() {
  // Given: a Scheduler facade
  let scheduler = new_scheduler();

  // When: schedule_with_fixed_delay_runnable is called with a closure-style runnable
  let result =
    scheduler.schedule_with_fixed_delay_runnable(Duration::from_millis(5), Duration::from_millis(20), noop_runnable);

  // Then: a SchedulerHandle is returned
  assert!(result.is_ok(), "schedule_with_fixed_delay_runnable should return a handle");
}
