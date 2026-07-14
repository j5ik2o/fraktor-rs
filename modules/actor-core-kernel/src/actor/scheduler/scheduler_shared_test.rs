use core::time::Duration;
use std::panic::{AssertUnwindSafe, catch_unwind};

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use super::SchedulerShared;
use crate::actor::scheduler::{SchedulerCommand, SchedulerConfig, SchedulerContext};

#[test]
fn run_after_write_allows_action_to_reenter_scheduler() {
  let context = SchedulerContext::new(SchedulerConfig::default());
  let scheduler = context.scheduler();
  let action_scheduler = scheduler.clone();
  let action_ran = ArcShared::new(SpinSyncMutex::new(false));
  let action_ran_for_action = action_ran.clone();

  scheduler.with_write(|_| {
    scheduler.run_after_write(move || {
      action_scheduler.with_write(|inner| {
        inner.schedule_once(Duration::from_millis(10), SchedulerCommand::Noop).expect("schedule after outer write");
      });
      *action_ran_for_action.lock() = true;
    });
    assert!(!*action_ran.lock(), "action must wait for the scheduler write lock to be released");
  });

  assert!(*action_ran.lock());
}

#[test]
fn run_after_write_runs_immediately_when_scheduler_is_idle() {
  let scheduler: SchedulerShared = SchedulerContext::new(SchedulerConfig::default()).scheduler();
  let action_ran = ArcShared::new(SpinSyncMutex::new(false));
  let action_ran_for_action = action_ran.clone();

  scheduler.run_after_write(move || {
    *action_ran_for_action.lock() = true;
  });

  assert!(*action_ran.lock());
}

#[test]
fn scheduler_write_panic_discards_deferred_actions_and_restores_idle_state() {
  let scheduler = SchedulerContext::new(SchedulerConfig::default()).scheduler();

  let result = catch_unwind(AssertUnwindSafe(|| {
    scheduler.with_write(|_| {
      scheduler.run_after_write(|| {});
      panic!("scheduler write failed");
    });
  }));

  assert!(result.is_err());
  let immediate_ran = ArcShared::new(SpinSyncMutex::new(false));
  let immediate_ran_for_action = immediate_ran.clone();
  scheduler.run_after_write(move || {
    *immediate_ran_for_action.lock() = true;
  });
  assert!(*immediate_ran.lock());
}
