use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeRwLock};

use crate::core::{
  kernel::{
    actor::actor_ref::ActorRef,
    scheduler::{Scheduler, SchedulerConfig, SchedulerShared},
  },
  typed::{
    actor::TypedActorRef, scheduler::TypedSchedulerShared, timer_key::TimerKey, timer_scheduler::TimerScheduler,
  },
};

fn build_scheduler_pair() -> (TypedSchedulerShared, TypedActorRef<u32>) {
  let config = SchedulerConfig::default();
  let scheduler = Scheduler::new(config);
  let rwlock = RuntimeRwLock::new(scheduler);
  let shared = SchedulerShared::new(ArcShared::new(rwlock));
  let typed_shared = TypedSchedulerShared::new(shared);
  let receiver = TypedActorRef::<u32>::from_untyped(ActorRef::null());
  (typed_shared, receiver)
}

#[test]
fn start_single_timer_registers_entry() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key = TimerKey::new("tick");
  ts.start_single_timer(key.clone(), 1u32, Duration::from_millis(100)).expect("schedule");
  assert!(ts.is_timer_active(&key));
}

#[test]
fn cancel_removes_timer() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key = TimerKey::new("tick");
  ts.start_single_timer(key.clone(), 1u32, Duration::from_millis(100)).expect("schedule");
  ts.cancel(&key);
  assert!(!ts.is_timer_active(&key));
}

#[test]
fn start_same_key_cancels_previous() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key = TimerKey::new("tick");
  ts.start_single_timer(key.clone(), 1u32, Duration::from_millis(100)).expect("schedule");
  ts.start_single_timer(key.clone(), 2u32, Duration::from_millis(200)).expect("schedule");
  assert!(ts.is_timer_active(&key));
}

#[test]
fn cancel_all_clears_entries() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key_a = TimerKey::new("a");
  let key_b = TimerKey::new("b");
  ts.start_single_timer(key_a.clone(), 1u32, Duration::from_millis(100)).expect("schedule");
  ts.start_single_timer(key_b.clone(), 2u32, Duration::from_millis(200)).expect("schedule");
  ts.cancel_all();
  assert!(!ts.is_timer_active(&key_a));
  assert!(!ts.is_timer_active(&key_b));
}

#[test]
fn start_timer_with_fixed_delay_registers_entry() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key = TimerKey::new("periodic");
  ts.start_timer_with_fixed_delay(key.clone(), 1u32, Duration::from_millis(100)).expect("schedule");
  assert!(ts.is_timer_active(&key));
}

#[test]
fn start_timer_with_fixed_delay_initial_registers_entry() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key = TimerKey::new("periodic_initial");
  ts.start_timer_with_fixed_delay_initial(key.clone(), 1u32, Duration::from_millis(50), Duration::from_millis(100))
    .expect("schedule");
  assert!(ts.is_timer_active(&key));
}

#[test]
fn start_timer_at_fixed_rate_registers_entry() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key = TimerKey::new("rate");
  ts.start_timer_at_fixed_rate(key.clone(), 1u32, Duration::from_millis(100)).expect("schedule");
  assert!(ts.is_timer_active(&key));
}

#[test]
fn start_timer_at_fixed_rate_initial_registers_entry() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key = TimerKey::new("rate_initial");
  ts.start_timer_at_fixed_rate_initial(key.clone(), 1u32, Duration::from_millis(50), Duration::from_millis(100))
    .expect("schedule");
  assert!(ts.is_timer_active(&key));
}
