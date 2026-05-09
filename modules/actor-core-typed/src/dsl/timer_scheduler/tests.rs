use core::time::Duration;

use fraktor_actor_core_kernel_rs::actor::{
  actor_ref::ActorRef,
  scheduler::{SchedulerConfig, SchedulerContext},
};

use crate::{
  TypedActorRef,
  dsl::{TimerKey, TimerScheduler},
  internal::TypedSchedulerShared,
};

fn build_scheduler_pair() -> (TypedSchedulerShared, TypedActorRef<u32>) {
  let context = SchedulerContext::new(SchedulerConfig::default());
  let typed_shared = TypedSchedulerShared::new(context.scheduler());
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
fn start_single_timer_with_message_key_replaces_previous_timer_for_same_message() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key = TimerScheduler::timer_key_for_message(&7u32);

  ts.start_single_timer_with_message_key(7u32, Duration::from_millis(100)).expect("schedule first");
  let first = ts.entries.get(&key).expect("first handle").clone();
  ts.start_single_timer_with_message_key(7u32, Duration::from_millis(200)).expect("schedule replacement");
  let second = ts.entries.get(&key).expect("replacement handle");

  assert_eq!(ts.entries.len(), 1);
  assert!(first.is_cancelled(), "replaced one-shot timer should be cancelled");
  assert_ne!(first.raw(), second.raw(), "replacement should install a fresh handle");
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
fn start_timer_with_fixed_delay_with_message_key_replaces_previous_timer_for_same_message() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key = TimerScheduler::timer_key_for_message(&11u32);

  ts.start_timer_with_fixed_delay_with_message_key(11u32, Duration::from_millis(100)).expect("schedule first");
  let first = ts.entries.get(&key).expect("first handle").clone();
  ts.start_timer_with_fixed_delay_with_message_key(11u32, Duration::from_millis(150)).expect("schedule replacement");
  let second = ts.entries.get(&key).expect("replacement handle");

  assert_eq!(ts.entries.len(), 1);
  assert!(first.is_cancelled(), "replaced fixed-delay timer should be cancelled");
  assert_ne!(first.raw(), second.raw(), "replacement should install a fresh handle");
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
fn start_timer_at_fixed_rate_with_message_key_replaces_previous_timer_for_same_message() {
  let (shared, receiver) = build_scheduler_pair();
  let mut ts = TimerScheduler::new(receiver, shared);
  let key = TimerScheduler::timer_key_for_message(&13u32);

  ts.start_timer_at_fixed_rate_with_message_key(13u32, Duration::from_millis(100)).expect("schedule first");
  let first = ts.entries.get(&key).expect("first handle").clone();
  ts.start_timer_at_fixed_rate_with_message_key(13u32, Duration::from_millis(200)).expect("schedule replacement");
  let second = ts.entries.get(&key).expect("replacement handle");

  assert_eq!(ts.entries.len(), 1);
  assert!(first.is_cancelled(), "replaced fixed-rate timer should be cancelled");
  assert_ne!(first.raw(), second.raw(), "replacement should install a fresh handle");
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
