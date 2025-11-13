use alloc::{
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::{i32, time::Duration};

use fraktor_utils_core_rs::{
  sync::{ArcShared, NoStdMutex},
  time::SchedulerCapacityProfile,
};

use super::{
  Scheduler, SchedulerConfig, SchedulerError, api, command::SchedulerCommand, execution_batch::ExecutionBatch,
  runner::SchedulerRunner,
};
use crate::{
  NoStdToolbox,
  actor_prim::{
    Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender},
  },
  error::SendError,
  messaging::AnyMessageGeneric,
};

fn build_scheduler() -> Scheduler<NoStdToolbox> {
  let toolbox = NoStdToolbox::default();
  let config = SchedulerConfig::default();
  Scheduler::new(toolbox, config)
}

fn build_scheduler_with_resolution(resolution: Duration) -> Scheduler<NoStdToolbox> {
  let toolbox = NoStdToolbox::new(resolution);
  let profile = SchedulerCapacityProfile::standard();
  let config = SchedulerConfig::new(resolution, profile);
  Scheduler::new(toolbox, config)
}

#[test]
fn schedule_once_rejects_zero_delay() {
  let mut scheduler = build_scheduler();
  let result = scheduler.schedule_once(Duration::ZERO);
  assert_eq!(result, Err(SchedulerError::InvalidDelay));
}

#[test]
fn schedule_once_rejects_duration_exceeding_tick_limit() {
  let mut scheduler = build_scheduler_with_resolution(Duration::from_nanos(1));
  let invalid = Duration::from_nanos((i32::MAX as u64) + 1);
  let result = scheduler.schedule_once(invalid);
  assert_eq!(result, Err(SchedulerError::InvalidDelay));
}

#[test]
fn schedule_once_returns_handle_for_valid_delay() {
  let mut scheduler = build_scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(10)).expect("handle");
  assert_ne!(handle.raw(), 0);
}

#[test]
fn fixed_rate_requires_positive_period() {
  let mut scheduler = build_scheduler();
  let result = scheduler.schedule_at_fixed_rate(Duration::from_millis(5), Duration::ZERO);
  assert_eq!(result, Err(SchedulerError::InvalidDelay));
}

#[test]
fn schedule_fixed_rate_registers_job() {
  let mut scheduler = build_scheduler();
  assert_eq!(scheduler.job_count_for_test(), 0);
  let handle = scheduler.schedule_at_fixed_rate(Duration::from_millis(5), Duration::from_millis(7)).expect("handle");
  assert_ne!(handle.raw(), 0);
  assert_eq!(scheduler.job_count_for_test(), 1);
}

#[test]
fn cancel_existing_job_returns_true() {
  let mut scheduler = build_scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(2)).expect("handle");
  assert!(scheduler.cancel(&handle));
  assert!(handle.is_cancelled());
  assert!(!scheduler.cancel(&handle));
}

#[test]
fn shutdown_prevents_new_jobs() {
  let mut scheduler = build_scheduler();
  scheduler.shutdown();
  let result = scheduler.schedule_once(Duration::from_millis(1));
  assert_eq!(result, Err(SchedulerError::Closed));
}

#[test]
fn handles_are_unique_across_registrations() {
  let mut scheduler = build_scheduler();
  let mut ids = Vec::new();
  for offset in 1..=5u64 {
    let handle = scheduler.schedule_once(Duration::from_millis(offset)).expect("handle");
    ids.push(handle.raw());
  }
  ids.sort_unstable();
  ids.dedup();
  assert_eq!(ids.len(), 5);
}

#[test]
fn capacity_limit_returns_error() {
  let toolbox = NoStdToolbox::default();
  let profile = SchedulerCapacityProfile::new("tiny", 1, 1, 1);
  let config = SchedulerConfig::new(Duration::from_millis(1), profile).with_max_pending_jobs(1);
  let mut scheduler = Scheduler::new(toolbox, config);
  scheduler.schedule_once(Duration::from_millis(1)).expect("first");
  let err = scheduler.schedule_once(Duration::from_millis(2)).expect_err("second should fail");
  assert_eq!(err, SchedulerError::Backpressured);
}

#[test]
fn schedule_command_records_send_message() {
  let mut scheduler = build_scheduler();
  let receiver = ActorRefGeneric::null();
  let message = AnyMessageGeneric::new(42u32);
  let handle = scheduler
    .schedule_command(Duration::from_millis(3), SchedulerCommand::SendMessage {
      receiver:   receiver.clone(),
      message:    message.clone(),
      dispatcher: None,
      sender:     None,
    })
    .expect("handle");
  match scheduler.command_for_test(&handle) {
    | Some(SchedulerCommand::SendMessage { receiver: target, message: stored, dispatcher, sender }) => {
      assert_eq!(target.pid(), receiver.pid());
      assert!(stored.payload().is::<u32>());
      assert!(dispatcher.is_none());
      assert!(sender.is_none());
    },
    | other => panic!("unexpected command: {:?}", other),
  }
}

#[test]
fn api_schedule_once_records_sender_metadata() {
  let mut scheduler = build_scheduler();
  let receiver = ActorRefGeneric::null();
  let sender = ActorRefGeneric::null();
  let message = AnyMessageGeneric::new("payload".to_string());
  let handle = api::schedule_once(
    &mut scheduler,
    Duration::from_millis(4),
    receiver.clone(),
    message.clone(),
    None,
    Some(sender.clone()),
  )
  .expect("handle");
  match scheduler.command_for_test(&handle) {
    | Some(SchedulerCommand::SendMessage { receiver: target, message: stored, dispatcher, sender: stored_sender }) => {
      assert_eq!(target.pid(), receiver.pid());
      assert!(stored.payload().is::<String>());
      assert!(dispatcher.is_none());
      assert_eq!(stored_sender.as_ref().map(ActorRefGeneric::pid), Some(sender.pid()));
    },
    | other => panic!("unexpected command: {:?}", other),
  }
}

#[test]
fn api_schedule_at_fixed_rate_executes_multiple_runs() {
  let mut scheduler = build_scheduler();
  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let sender = ArcShared::new(RecordingSender { inbox: inbox.clone() });
  let receiver = ActorRefGeneric::new(Pid::new(2, 0), sender);
  api::schedule_at_fixed_rate(
    &mut scheduler,
    Duration::from_millis(2),
    Duration::from_millis(3),
    receiver,
    AnyMessageGeneric::new(11u32),
    None,
    None,
  )
  .expect("handle");
  scheduler.run_for_test(2);
  scheduler.run_for_test(3);
  scheduler.run_for_test(3);
  assert_eq!(inbox.lock().len(), 3);
}

#[test]
fn api_schedule_with_fixed_delay_rejects_zero_initial_delay() {
  let mut scheduler = build_scheduler();
  let receiver = ActorRefGeneric::null();
  let err = api::schedule_with_fixed_delay(
    &mut scheduler,
    Duration::ZERO,
    Duration::from_millis(1),
    receiver,
    AnyMessageGeneric::new(1u32),
    None,
    None,
  )
  .expect_err("should reject zero initial delay");
  assert_eq!(err, SchedulerError::InvalidDelay);
}

#[test]
fn schedule_once_fn_executes_runnable() {
  let mut scheduler = build_scheduler();
  let counter = ArcShared::new(NoStdMutex::new(0usize));
  let captured = counter.clone();
  api::schedule_once_fn(&mut scheduler, Duration::from_millis(1), None, move |batch: &ExecutionBatch| {
    assert_eq!(batch.runs().get(), 1);
    let mut guard = captured.lock();
    *guard += 1;
  })
  .expect("handle");
  scheduler.run_for_test(1);
  assert_eq!(*counter.lock(), 1);
}

#[test]
fn run_for_test_executes_send_message() {
  let mut scheduler = build_scheduler();
  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let sender = ArcShared::new(RecordingSender { inbox: inbox.clone() });
  let receiver = ActorRefGeneric::new(Pid::new(1, 0), sender);
  api::schedule_once(&mut scheduler, Duration::from_millis(5), receiver, AnyMessageGeneric::new(7u32), None, None)
    .expect("handle");
  scheduler.run_for_test(5);
  assert_eq!(inbox.lock().len(), 1);
}

#[test]
fn schedule_once_fn_records_execution_batch() {
  let mut scheduler = build_scheduler();
  let observed = ArcShared::new(NoStdMutex::new(Vec::new()));
  let capture = observed.clone();
  api::schedule_once_fn(&mut scheduler, Duration::from_millis(1), None, move |batch: &ExecutionBatch| {
    capture.lock().push((batch.runs().get(), batch.missed_runs()));
  })
  .expect("handle");
  scheduler.run_for_test(1);
  let guard = observed.lock();
  assert_eq!(guard.len(), 1);
  assert_eq!(guard[0], (1, 0));
}

#[test]
fn runner_manual_processes_ticks_in_order() {
  let mut scheduler = build_scheduler();
  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let sender = ArcShared::new(RecordingSender { inbox: inbox.clone() });
  let receiver = ActorRefGeneric::new(Pid::new(7, 0), sender);

  api::schedule_once(
    &mut scheduler,
    Duration::from_millis(1),
    receiver.clone(),
    AnyMessageGeneric::new(1u32),
    None,
    None,
  )
  .expect("handle");
  api::schedule_once(&mut scheduler, Duration::from_millis(1), receiver, AnyMessageGeneric::new(2u32), None, None)
    .expect("handle");

  {
    let mut runner = SchedulerRunner::manual(&mut scheduler);
    runner.inject_manual_ticks(1);
    runner.run_once();
  }

  let delivered: Vec<u32> =
    inbox.lock().iter().map(|msg| *msg.payload().downcast_ref::<u32>().expect("u32 payload")).collect();
  assert_eq!(delivered, vec![1, 2]);
}

#[test]
fn backpressure_error_returned_when_pending_jobs_exceed_limit() {
  let toolbox = NoStdToolbox::default();
  let profile = SchedulerCapacityProfile::new("tiny", 32, 8, 4);
  let config = SchedulerConfig::new(Duration::from_millis(1), profile).with_max_pending_jobs(1);
  let mut scheduler = Scheduler::new(toolbox, config);
  scheduler.schedule_once(Duration::from_millis(1)).expect("first");
  let err = scheduler.schedule_once(Duration::from_millis(2)).expect_err("second");
  assert_eq!(err, SchedulerError::Backpressured);
}

#[test]
fn handle_reports_cancelled_state() {
  let mut scheduler = build_scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(5)).expect("handle");
  assert!(!handle.is_cancelled());
  assert!(scheduler.cancel(&handle));
  assert!(handle.is_cancelled());
  assert!(!scheduler.cancel(&handle));
}

#[test]
fn cancelled_job_is_not_delivered() {
  let mut scheduler = build_scheduler();
  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let sender = ArcShared::new(RecordingSender { inbox: inbox.clone() });
  let receiver = ActorRefGeneric::new(Pid::new(3, 0), sender);
  let handle =
    api::schedule_once(&mut scheduler, Duration::from_millis(2), receiver, AnyMessageGeneric::new(42u32), None, None)
      .expect("handle");
  assert!(scheduler.cancel(&handle));
  scheduler.run_for_test(2);
  assert_eq!(inbox.lock().len(), 0);
}

#[test]
fn handle_reports_completed_after_execution() {
  let mut scheduler = build_scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(3)).expect("handle");
  scheduler.run_for_test(3);
  assert!(handle.is_completed());
  assert!(!handle.is_cancelled());
}

#[test]
fn scheduler_metrics_track_active_and_drops() {
  let mut scheduler = build_scheduler();
  let first = scheduler.schedule_once(Duration::from_millis(2)).expect("first");
  let second = scheduler.schedule_once(Duration::from_millis(4)).expect("second");
  assert_eq!(scheduler.metrics().active_timers(), 2);
  assert!(scheduler.cancel(&second));
  scheduler.run_for_test(2);
  assert!(first.is_completed());
  assert_eq!(scheduler.metrics().active_timers(), 0);
  assert_eq!(scheduler.metrics().dropped_total(), 1);
}

#[test]
fn fixed_rate_handle_can_be_cancelled_after_multiple_runs() {
  let mut scheduler = build_scheduler();
  let handle = scheduler.schedule_at_fixed_rate(Duration::from_millis(1), Duration::from_millis(1)).expect("handle");
  scheduler.run_for_test(1);
  scheduler.run_for_test(1);
  assert!(!handle.is_completed());
  assert!(scheduler.cancel(&handle));
  assert!(handle.is_cancelled());
}

struct RecordingSender {
  inbox: ArcShared<NoStdMutex<Vec<AnyMessageGeneric<NoStdToolbox>>>>,
}

impl ActorRefSender<NoStdToolbox> for RecordingSender {
  fn send(&self, message: AnyMessageGeneric<NoStdToolbox>) -> Result<(), SendError<NoStdToolbox>> {
    self.inbox.lock().push(message);
    Ok(())
  }
}
