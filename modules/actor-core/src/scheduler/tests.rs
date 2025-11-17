use alloc::{
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::{
  cmp,
  num::NonZeroU32,
  pin::Pin,
  task::{Context, Poll},
  time::Duration,
};

use fraktor_utils_core_rs::core::{
  runtime_toolbox::SyncMutexFamily,
  sync::{ArcShared, NoStdMutex},
  time::{SchedulerCapacityProfile, SchedulerTickHandle},
  timing::{DelayFuture, DelayProvider},
};
use hashbrown::HashMap;
use proptest::prelude::*;

use super::{
  BatchMode, DeterministicEvent, ExecutionBatch, Scheduler, SchedulerBackedDelayProvider, SchedulerConfig,
  SchedulerContext, SchedulerDiagnosticsEvent, SchedulerError, SchedulerMode, SchedulerRunnable, SchedulerRunner,
  SchedulerWarning, TaskRunError, TaskRunOnClose, TaskRunPriority, command::SchedulerCommand,
  fixed_delay_policy::FixedDelayPolicy, fixed_rate_policy::FixedRatePolicy, handle::SchedulerHandle,
};
use crate::{
  NoStdToolbox, RuntimeToolbox, ToolboxMutex,
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

fn build_scheduler_with_policies(
  rate_policy: FixedRatePolicy,
  delay_policy: FixedDelayPolicy,
) -> Scheduler<NoStdToolbox> {
  let toolbox = NoStdToolbox::default();
  let profile = SchedulerCapacityProfile::standard();
  let config = SchedulerConfig::new(Duration::from_millis(1), profile)
    .with_fixed_rate_policy(rate_policy)
    .with_fixed_delay_policy(delay_policy);
  Scheduler::new(toolbox, config)
}

fn nz(value: u32) -> NonZeroU32 {
  NonZeroU32::new(value).expect("non-zero")
}

#[derive(Clone)]
struct RecordingTask {
  log:        ArcShared<NoStdMutex<Vec<&'static str>>>,
  label:      &'static str,
  should_err: bool,
}

impl RecordingTask {
  fn succeed(log: ArcShared<NoStdMutex<Vec<&'static str>>>, label: &'static str) -> ArcShared<Self> {
    ArcShared::new(Self { log, label, should_err: false })
  }

  fn fail(log: ArcShared<NoStdMutex<Vec<&'static str>>>, label: &'static str) -> ArcShared<Self> {
    ArcShared::new(Self { log, label, should_err: true })
  }
}

impl TaskRunOnClose for RecordingTask {
  fn run(&self) -> Result<(), TaskRunError> {
    self.log.lock().push(self.label);
    if self.should_err { Err(TaskRunError::new("fail")) } else { Ok(()) }
  }
}

type SharedScheduler = ArcShared<ToolboxMutex<Scheduler<NoStdToolbox>, NoStdToolbox>>;

fn shared_scheduler_state() -> (SharedScheduler, SchedulerBackedDelayProvider<NoStdToolbox>) {
  let toolbox = NoStdToolbox::default();
  let scheduler = Scheduler::new(toolbox, SchedulerConfig::default());
  let mutex = <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(scheduler);
  let shared = ArcShared::new(mutex);
  let provider = SchedulerBackedDelayProvider::new(shared.clone());
  (shared, provider)
}

fn noop_waker() -> core::task::Waker {
  use core::task::{RawWaker, RawWakerVTable, Waker};

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

fn schedule_message_command<TB: RuntimeToolbox>(
  scheduler: &mut Scheduler<TB>,
  delay: Duration,
  receiver: ActorRefGeneric<TB>,
  message: AnyMessageGeneric<TB>,
  sender: Option<ActorRefGeneric<TB>>,
) -> Result<SchedulerHandle, SchedulerError> {
  scheduler.schedule_command(delay, SchedulerCommand::SendMessage { receiver, message, dispatcher: None, sender })
}

fn schedule_runnable_command<TB, F>(
  scheduler: &mut Scheduler<TB>,
  delay: Duration,
  runnable: F,
) -> Result<SchedulerHandle, SchedulerError>
where
  TB: RuntimeToolbox,
  F: SchedulerRunnable, {
  let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(runnable);
  scheduler.schedule_command(delay, SchedulerCommand::RunRunnable { runnable, dispatcher: None })
}

struct ManualRunnerOwner;

#[test]
fn schedule_once_rejects_zero_delay() {
  let mut scheduler = build_scheduler();
  let result = scheduler.schedule_once(Duration::ZERO, SchedulerCommand::Noop);
  assert_eq!(result, Err(SchedulerError::InvalidDelay));
}

#[test]
fn schedule_once_rejects_duration_exceeding_tick_limit() {
  let mut scheduler = build_scheduler_with_resolution(Duration::from_nanos(1));
  let invalid = Duration::from_nanos((i32::MAX as u64) + 1);
  let result = scheduler.schedule_once(invalid, SchedulerCommand::Noop);
  assert_eq!(result, Err(SchedulerError::InvalidDelay));
}

#[test]
fn schedule_once_returns_handle_for_valid_delay() {
  let mut scheduler = build_scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(10), SchedulerCommand::Noop).expect("handle");
  assert_ne!(handle.raw(), 0);
}

#[test]
fn fixed_rate_requires_positive_period() {
  let mut scheduler = build_scheduler();
  let result = scheduler.schedule_at_fixed_rate(Duration::from_millis(5), Duration::ZERO, SchedulerCommand::Noop);
  assert_eq!(result, Err(SchedulerError::InvalidDelay));
}

#[test]
fn schedule_fixed_rate_registers_job() {
  let mut scheduler = build_scheduler();
  assert_eq!(scheduler.job_count_for_test(), 0);
  let handle = scheduler
    .schedule_at_fixed_rate(Duration::from_millis(5), Duration::from_millis(7), SchedulerCommand::Noop)
    .expect("handle");
  assert_ne!(handle.raw(), 0);
  assert_eq!(scheduler.job_count_for_test(), 1);
}

#[test]
fn cancel_existing_job_returns_true() {
  let mut scheduler = build_scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(2), SchedulerCommand::Noop).expect("handle");
  assert!(scheduler.cancel(&handle));
  assert!(handle.is_cancelled());
  assert!(!scheduler.cancel(&handle));
}

#[test]
fn shutdown_prevents_new_jobs() {
  let mut scheduler = build_scheduler();
  let _ = scheduler.shutdown();
  let result = scheduler.schedule_once(Duration::from_millis(1), SchedulerCommand::Noop);
  assert_eq!(result, Err(SchedulerError::Closed));
}

#[test]
fn handles_are_unique_across_registrations() {
  let mut scheduler = build_scheduler();
  let mut ids = Vec::new();
  for offset in 1..=5u64 {
    let handle = scheduler.schedule_once(Duration::from_millis(offset), SchedulerCommand::Noop).expect("handle");
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
  scheduler.schedule_once(Duration::from_millis(1), SchedulerCommand::Noop).expect("first");
  let err = scheduler.schedule_once(Duration::from_millis(2), SchedulerCommand::Noop).expect_err("second should fail");
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
fn schedule_once_records_sender_metadata() {
  let mut scheduler = build_scheduler();
  let receiver = ActorRefGeneric::null();
  let sender = ActorRefGeneric::null();
  let message = AnyMessageGeneric::new("payload".to_string());
  let handle = schedule_message_command(
    &mut scheduler,
    Duration::from_millis(4),
    receiver.clone(),
    message.clone(),
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
fn schedule_at_fixed_rate_executes_multiple_runs() {
  let mut scheduler = build_scheduler();
  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let sender = ArcShared::new(RecordingSender { inbox: inbox.clone() });
  let receiver = ActorRefGeneric::new(Pid::new(2, 0), sender);
  scheduler
    .schedule_at_fixed_rate(Duration::from_millis(2), Duration::from_millis(3), SchedulerCommand::SendMessage {
      receiver,
      message: AnyMessageGeneric::new(11u32),
      dispatcher: None,
      sender: None,
    })
    .expect("handle");
  scheduler.run_for_test(2);
  scheduler.run_for_test(3);
  scheduler.run_for_test(3);
  assert_eq!(inbox.lock().len(), 3);
}

#[test]
fn schedule_with_fixed_delay_rejects_zero_initial_delay() {
  let mut scheduler = build_scheduler();
  let err = scheduler
    .schedule_with_fixed_delay(Duration::ZERO, Duration::from_millis(1), SchedulerCommand::Noop)
    .expect_err("should reject zero initial delay");
  assert_eq!(err, SchedulerError::InvalidDelay);
}

#[test]
fn schedule_once_fn_executes_runnable() {
  let mut scheduler = build_scheduler();
  let counter = ArcShared::new(NoStdMutex::new(0usize));
  let captured = counter.clone();
  schedule_runnable_command(&mut scheduler, Duration::from_millis(1), move |batch: &ExecutionBatch| {
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
  schedule_message_command(&mut scheduler, Duration::from_millis(5), receiver, AnyMessageGeneric::new(7u32), None)
    .expect("handle");
  scheduler.run_for_test(5);
  assert_eq!(inbox.lock().len(), 1);
}

#[test]
fn schedule_once_fn_records_execution_batch() {
  let mut scheduler = build_scheduler();
  let observed = ArcShared::new(NoStdMutex::new(Vec::new()));
  let capture = observed.clone();
  schedule_runnable_command(&mut scheduler, Duration::from_millis(1), move |batch: &ExecutionBatch| {
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

  schedule_message_command(
    &mut scheduler,
    Duration::from_millis(1),
    receiver.clone(),
    AnyMessageGeneric::new(1u32),
    None,
  )
  .expect("handle");
  schedule_message_command(&mut scheduler, Duration::from_millis(1), receiver, AnyMessageGeneric::new(2u32), None)
    .expect("handle");

  {
    let owner = ManualRunnerOwner;
    let tick_handle = SchedulerTickHandle::scoped(&owner);
    let mut runner = SchedulerRunner::manual(&tick_handle);
    runner.inject_manual_ticks(1);
    runner.run_once(&mut scheduler);
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
  scheduler.schedule_once(Duration::from_millis(1), SchedulerCommand::Noop).expect("first");
  let err = scheduler.schedule_once(Duration::from_millis(2), SchedulerCommand::Noop).expect_err("second");
  assert_eq!(err, SchedulerError::Backpressured);
}

#[test]
fn timer_wheel_capacity_exceeded_returns_error() {
  let toolbox = NoStdToolbox::default();
  let profile = SchedulerCapacityProfile::new("mini", 1, 1, 1);
  let config = SchedulerConfig::new(Duration::from_millis(1), profile).with_max_pending_jobs(2);
  let mut scheduler = Scheduler::new(toolbox, config);
  scheduler.schedule_once(Duration::from_millis(1), SchedulerCommand::Noop).expect("first");
  let err = scheduler.schedule_once(Duration::from_millis(2), SchedulerCommand::Noop).expect_err("second");
  assert_eq!(err, SchedulerError::CapacityExceeded);
}

#[test]
fn handle_reports_cancelled_state() {
  let mut scheduler = build_scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(5), SchedulerCommand::Noop).expect("handle");
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
    schedule_message_command(&mut scheduler, Duration::from_millis(2), receiver, AnyMessageGeneric::new(42u32), None)
      .expect("handle");
  assert!(scheduler.cancel(&handle));
  scheduler.run_for_test(2);
  assert_eq!(inbox.lock().len(), 0);
}

#[test]
fn handle_reports_completed_after_execution() {
  let mut scheduler = build_scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(3), SchedulerCommand::Noop).expect("handle");
  scheduler.run_for_test(3);
  assert!(handle.is_completed());
  assert!(!handle.is_cancelled());
}

#[test]
fn scheduler_metrics_track_active_and_drops() {
  let mut scheduler = build_scheduler();
  let first = scheduler.schedule_once(Duration::from_millis(2), SchedulerCommand::Noop).expect("first");
  let second = scheduler.schedule_once(Duration::from_millis(4), SchedulerCommand::Noop).expect("second");
  assert_eq!(scheduler.metrics().active_timers(), 2);
  assert!(scheduler.cancel(&second));
  scheduler.run_for_test(2);
  assert!(first.is_completed());
  assert_eq!(scheduler.metrics().active_timers(), 0);
  assert_eq!(scheduler.metrics().dropped_total(), 1);
}

#[test]
fn fixed_rate_runnable_reports_missed_runs() {
  let toolbox = NoStdToolbox::new(Duration::from_millis(1));
  let profile = SchedulerCapacityProfile::standard();
  let config = SchedulerConfig::new(Duration::from_millis(1), profile).with_backlog_limit(10);
  let mut scheduler = Scheduler::new(toolbox, config);
  let batches = ArcShared::new(NoStdMutex::new(Vec::new()));
  let capture = batches.clone();
  let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(move |batch: &ExecutionBatch| {
    capture.lock().push((batch.mode(), batch.runs().get(), batch.missed_runs()));
  });
  scheduler
    .schedule_at_fixed_rate(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::RunRunnable {
      runnable:   runnable.clone(),
      dispatcher: None,
    })
    .expect("handle");

  scheduler.run_for_test(1);
  scheduler.run_for_test(5);

  let guard = batches.lock();
  assert_eq!(guard.len(), 2, "batches: {:?}", *guard);
  assert_eq!(guard[0], (BatchMode::FixedRate, 1, 0));
  assert_eq!(guard[1].0, BatchMode::FixedRate);
  assert!(guard[1].1 > 1, "batches: {:?}", *guard);
  assert!(guard[1].2 > 0, "batches: {:?}", *guard);
}

#[test]
fn backlog_limit_auto_cancels_periodic_job() {
  let toolbox = NoStdToolbox::default();
  let config = SchedulerConfig::default().with_backlog_limit(1);
  let mut scheduler = Scheduler::new(toolbox, config);
  scheduler
    .schedule_at_fixed_rate(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("handle");
  scheduler.run_for_test(10);
  assert_eq!(scheduler.metrics().active_timers(), 0);
  assert_eq!(scheduler.warnings().len(), 1);
}

#[test]
fn fixed_rate_handle_can_be_cancelled_after_multiple_runs() {
  let mut scheduler = build_scheduler();
  let handle = scheduler
    .schedule_at_fixed_rate(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("handle");
  scheduler.run_for_test(1);
  scheduler.run_for_test(1);
  assert!(!handle.is_completed());
  assert!(scheduler.cancel(&handle));
  assert!(handle.is_cancelled());
}

#[test]
fn fixed_rate_policy_enforces_independent_backlog_limit() {
  let rate_policy = FixedRatePolicy::new(nz(1), nz(8));
  let delay_policy = FixedDelayPolicy::new(nz(8), nz(16));
  let mut scheduler = build_scheduler_with_policies(rate_policy, delay_policy);
  let rate_handle = scheduler
    .schedule_at_fixed_rate(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("rate");
  let delay_handle = scheduler
    .schedule_with_fixed_delay(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("delay");
  scheduler.run_for_test(5);
  assert!(rate_handle.is_cancelled(), "fixed-rate handle should cancel once backlog limit is exceeded");
  assert!(
    scheduler
      .warnings()
      .iter()
      .any(|warning| matches!(warning, SchedulerWarning::BacklogExceeded { handle_id, .. } if *handle_id == rate_handle.raw())),
    "expected backlog warning for fixed-rate handle",
  );
  assert!(!delay_handle.is_cancelled(), "fixed-delay handle should remain active because its backlog limit is relaxed",);
}

#[test]
fn fixed_delay_policy_enforces_independent_backlog_limit() {
  let rate_policy = FixedRatePolicy::new(nz(8), nz(16));
  let delay_policy = FixedDelayPolicy::new(nz(1), nz(8));
  let profile = SchedulerCapacityProfile::standard();
  let config = SchedulerConfig::new(Duration::from_millis(1), profile)
    .with_fixed_delay_policy(delay_policy)
    .with_fixed_rate_policy(rate_policy);
  let mut scheduler = Scheduler::new(NoStdToolbox::default(), config);
  let rate_handle = scheduler
    .schedule_at_fixed_rate(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("rate");
  let delay_handle = scheduler
    .schedule_with_fixed_delay(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("delay");
  scheduler.run_for_test(5);
  assert!(
    scheduler
      .warnings()
      .iter()
      .any(|warning| matches!(warning, SchedulerWarning::BacklogExceeded { handle_id, .. } if *handle_id == delay_handle.raw())),
    "expected backlog warning for fixed-delay handle",
  );
  assert!(delay_handle.is_cancelled(), "fixed-delay handle should cancel once backlog limit is exceeded");
  assert!(
    !scheduler
      .warnings()
      .iter()
      .any(|warning| matches!(warning, SchedulerWarning::BacklogExceeded { handle_id, .. } if *handle_id == rate_handle.raw())),
    "fixed-rate handle should not be cancelled when its policy allows larger backlog",
  );
}

#[test]
fn fixed_rate_policy_controls_burst_threshold() {
  let rate_policy = FixedRatePolicy::new(nz(8), nz(1));
  let delay_policy = FixedDelayPolicy::new(nz(8), nz(16));
  let mut scheduler = build_scheduler_with_policies(rate_policy, delay_policy);
  let rate_handle = scheduler
    .schedule_at_fixed_rate(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("rate");
  let delay_handle = scheduler
    .schedule_with_fixed_delay(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("delay");
  scheduler.run_for_test(4);
  assert!(
    scheduler.warnings().iter().any(
      |warning| matches!(warning, SchedulerWarning::BurstFire { handle_id, .. } if *handle_id == rate_handle.raw())
    ),
    "fixed-rate job should emit burst warning when threshold is exceeded",
  );
  assert!(
    !scheduler.warnings().iter().any(
      |warning| matches!(warning, SchedulerWarning::BurstFire { handle_id, .. } if *handle_id == delay_handle.raw())
    ),
    "fixed-delay job should not emit burst warning when its threshold is not exceeded",
  );
}

#[test]
fn fixed_delay_policy_controls_burst_threshold() {
  let rate_policy = FixedRatePolicy::new(nz(8), nz(16));
  let delay_policy = FixedDelayPolicy::new(nz(8), nz(1));
  let profile = SchedulerCapacityProfile::standard();
  let config = SchedulerConfig::new(Duration::from_millis(1), profile)
    .with_fixed_delay_policy(delay_policy)
    .with_fixed_rate_policy(rate_policy);
  let mut scheduler = Scheduler::new(NoStdToolbox::default(), config);
  let rate_handle = scheduler
    .schedule_at_fixed_rate(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("rate");
  let delay_handle = scheduler
    .schedule_with_fixed_delay(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("delay");
  scheduler.run_for_test(4);
  assert!(
    scheduler.warnings().iter().any(
      |warning| matches!(warning, SchedulerWarning::BurstFire { handle_id, .. } if *handle_id == delay_handle.raw())
    ),
    "fixed-delay job should emit burst warning when its threshold is exceeded",
  );
  assert!(
    !scheduler.warnings().iter().any(
      |warning| matches!(warning, SchedulerWarning::BurstFire { handle_id, .. } if *handle_id == rate_handle.raw())
    ),
    "fixed-rate job should remain silent when its threshold is higher",
  );
}

#[test]
fn fixed_rate_backlog_marks_handle_cancelled() {
  let profile = SchedulerCapacityProfile::standard();
  let config = SchedulerConfig::new(Duration::from_millis(1), profile).with_backlog_limit(1);
  let mut scheduler = Scheduler::new(NoStdToolbox::default(), config);
  let handle = scheduler
    .schedule_at_fixed_rate(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("handle");
  scheduler.run_for_test(5);
  assert!(handle.is_cancelled(), "fixed-rate handle should report cancelled after backlog drop");
}

#[test]
fn fixed_delay_backlog_marks_handle_cancelled() {
  let profile = SchedulerCapacityProfile::standard();
  let config = SchedulerConfig::new(Duration::from_millis(1), profile).with_backlog_limit(1);
  let mut scheduler = Scheduler::new(NoStdToolbox::default(), config);
  let handle = scheduler
    .schedule_with_fixed_delay(Duration::from_millis(1), Duration::from_millis(1), SchedulerCommand::Noop)
    .expect("handle");
  scheduler.run_for_test(5);
  assert!(handle.is_cancelled(), "fixed-delay handle should report cancelled after backlog drop");
}

#[test]
fn scheduler_backed_delay_provider_completes_future() {
  let (shared, provider) = shared_scheduler_state();
  let mut future = provider.delay(Duration::from_millis(1));
  assert!(matches!(poll_delay_future(&mut future), Poll::Pending));

  {
    let mut guard = shared.lock();
    guard.run_for_test(1);
  }

  assert!(matches!(poll_delay_future(&mut future), Poll::Ready(())));
}

#[test]
fn scheduler_backed_delay_provider_cancels_on_drop() {
  let (shared, provider) = shared_scheduler_state();
  let future = provider.delay(Duration::from_millis(5));
  drop(future);

  let guard = shared.lock();
  assert_eq!(guard.metrics().active_timers(), 0, "timer should be cancelled when future is dropped");
}

#[test]
fn scheduler_context_provides_shared_delay_provider() {
  let service = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let mut future = service.delay_provider().delay(Duration::from_millis(1));
  assert!(matches!(poll_delay_future(&mut future), Poll::Pending));

  {
    let scheduler = service.scheduler();
    let mut guard = scheduler.lock();
    guard.run_for_test(1);
  }

  assert!(matches!(poll_delay_future(&mut future), Poll::Ready(())));
}

#[derive(Clone, Debug)]
enum DeterministicOp {
  Schedule { delay_ticks: u16 },
  CancelLatest,
  Advance { ticks: u8 },
}

#[derive(Clone, Debug)]
enum RunnerOp {
  Schedule { delay_ticks: u16 },
  CancelLatest,
  Drive { ticks: u8 },
}

fn deterministic_op_strategy() -> impl Strategy<Value = DeterministicOp> {
  prop_oneof![
    (1u16..16).prop_map(|delay_ticks| DeterministicOp::Schedule { delay_ticks }),
    Just(DeterministicOp::CancelLatest),
    (1u8..8).prop_map(|ticks| DeterministicOp::Advance { ticks }),
  ]
}

fn runner_op_strategy() -> impl Strategy<Value = RunnerOp> {
  prop_oneof![
    (1u16..16).prop_map(|delay_ticks| RunnerOp::Schedule { delay_ticks }),
    Just(RunnerOp::CancelLatest),
    (1u8..8).prop_map(|ticks| RunnerOp::Drive { ticks }),
  ]
}

proptest! {
  #[test]
  fn deterministic_log_invariants(ops in prop::collection::vec(deterministic_op_strategy(), 1..32)) {
    let mut scheduler = build_scheduler();
    scheduler.enable_deterministic_log(1024);
    let mut handles: Vec<SchedulerHandle> = Vec::new();

    for op in ops {
      match op {
        DeterministicOp::Schedule { delay_ticks } => {
          let delay = Duration::from_millis((delay_ticks as u64).max(1));
          if let Ok(handle) = scheduler.schedule_once(delay, SchedulerCommand::Noop) {
            handles.push(handle);
          }
        },
        DeterministicOp::CancelLatest => {
          if let Some(handle) = handles.pop() {
            let _ = scheduler.cancel(&handle);
          }
        },
        DeterministicOp::Advance { ticks } => {
          scheduler.run_for_test(u64::from(ticks));
        },
      }
    }

    scheduler.run_for_test(64);
    assert_deterministic_invariants(scheduler.diagnostics().deterministic_log());
  }
}

proptest! {
  #[test]
  fn manual_runner_preserves_invariants(ops in prop::collection::vec(runner_op_strategy(), 1..32)) {
    let mut scheduler = build_scheduler();
    scheduler.enable_deterministic_log(1024);
    let owner = ManualRunnerOwner;
    let tick_handle = SchedulerTickHandle::scoped(&owner);
    let mut runner = SchedulerRunner::manual(&tick_handle);
    let mut handles: Vec<SchedulerHandle> = Vec::new();

    for op in ops {
      match op {
        RunnerOp::Schedule { delay_ticks } => {
          let delay = Duration::from_millis((delay_ticks as u64).max(1));
          if let Ok(handle) = scheduler.schedule_once(delay, SchedulerCommand::Noop) {
            handles.push(handle);
          }
        },
        RunnerOp::CancelLatest => {
          if let Some(handle) = handles.pop() {
            let _ = scheduler.cancel(&handle);
          }
        },
        RunnerOp::Drive { ticks } => {
          runner.inject_manual_ticks(u32::from(ticks));
          runner.run_once(&mut scheduler);
        },
      }
    }

    runner.inject_manual_ticks(32);
    runner.run_once(&mut scheduler);
    assert_deterministic_invariants(scheduler.diagnostics().deterministic_log());
  }
}

#[test]
fn deterministic_log_replay_matches_snapshot() {
  let mut scheduler = build_scheduler();
  scheduler.enable_deterministic_log(32);
  schedule_message_command(
    &mut scheduler,
    Duration::from_millis(2),
    ActorRefGeneric::null(),
    AnyMessageGeneric::new(5u32),
    None,
  )
  .expect("handle");
  scheduler.run_for_test(2);

  let replay_events: Vec<DeterministicEvent> = scheduler.diagnostics().replay().collect();
  assert_eq!(replay_events, scheduler.diagnostics().deterministic_log());
}

#[test]
fn diagnostics_subscription_receives_events() {
  let mut scheduler = build_scheduler();
  let mut subscription = scheduler.subscribe_diagnostics(8);
  let receiver = ActorRefGeneric::null();
  let handle =
    schedule_message_command(&mut scheduler, Duration::from_millis(2), receiver, AnyMessageGeneric::new(11u32), None)
      .expect("handle");
  scheduler.run_for_test(2);
  let events = subscription.drain();
  assert!(events.iter().any(
    |event| matches!(event, SchedulerDiagnosticsEvent::Scheduled { handle_id, .. } if *handle_id == handle.raw())
  ));
  assert!(
    events
      .iter()
      .any(|event| matches!(event, SchedulerDiagnosticsEvent::Fired { handle_id, .. } if *handle_id == handle.raw()))
  );
}

#[test]
fn diagnostics_drop_emits_warning() {
  let mut scheduler = build_scheduler();
  let mut _subscription = scheduler.subscribe_diagnostics(1);
  let receiver = ActorRefGeneric::null();
  schedule_message_command(
    &mut scheduler,
    Duration::from_millis(5),
    receiver.clone(),
    AnyMessageGeneric::new(1u32),
    None,
  )
  .expect("first");
  schedule_message_command(&mut scheduler, Duration::from_millis(6), receiver, AnyMessageGeneric::new(2u32), None)
    .expect("second");
  assert!(scheduler.warnings().iter().any(|warning| matches!(warning, SchedulerWarning::DiagnosticsDropped { .. })));
}

#[test]
fn diagnostics_replays_buffered_events_for_new_subscriber() {
  let mut scheduler = build_scheduler();
  let receiver = ActorRefGeneric::null();
  schedule_message_command(&mut scheduler, Duration::from_millis(2), receiver, AnyMessageGeneric::new(22u32), None)
    .expect("handle");
  scheduler.run_for_test(2);
  let mut subscription = scheduler.subscribe_diagnostics(4);
  let events = subscription.drain();
  assert!(events.iter().any(|event| matches!(event, SchedulerDiagnosticsEvent::Fired { .. })));
}

#[test]
fn scheduler_dump_reports_pending_jobs() {
  let mut scheduler = build_scheduler();
  let handle = scheduler.schedule_once(Duration::from_millis(5), SchedulerCommand::Noop).expect("handle");
  let dump = scheduler.dump();
  assert!(dump.jobs().iter().any(|job| job.handle_id() == handle.raw()));
}

#[test]
fn scheduler_dump_includes_periodic_metadata() {
  let mut scheduler = build_scheduler();
  let handle = scheduler
    .schedule_at_fixed_rate(Duration::from_millis(2), Duration::from_millis(4), SchedulerCommand::Noop)
    .expect("handle");
  let dump = scheduler.dump();
  let periodic = dump.jobs().iter().find(|job| job.handle_id() == handle.raw()).expect("job");
  assert_eq!(periodic.mode(), SchedulerMode::FixedRate);
  assert!(periodic.next_tick().is_some());
}

#[derive(Debug)]
struct StressReport {
  completed:     usize,
  max_drift_pct: u64,
}

fn run_stress_profile(job_count: usize, drift_ticks: u64) -> StressReport {
  let profile = SchedulerCapacityProfile::standard();
  let config = SchedulerConfig::new(Duration::from_millis(1), profile).with_max_pending_jobs(job_count + 512);
  let mut scheduler = Scheduler::new(NoStdToolbox::default(), config);
  scheduler.enable_deterministic_log(job_count * 4);

  for idx in 0..job_count {
    let delay = Duration::from_millis(((idx % 32) + 1) as u64);
    scheduler.schedule_once(delay, SchedulerCommand::Noop).expect("handle");
  }

  let total_ticks = 64 + drift_ticks;
  for _ in 0..total_ticks {
    scheduler.run_for_test(1);
  }

  let mut scheduled_deadlines = HashMap::new();
  let mut completed = 0usize;
  let mut max_drift_pct = 0u64;

  for event in scheduler.diagnostics().deterministic_log() {
    match *event {
      | DeterministicEvent::Scheduled { handle_id, deadline_tick, .. } => {
        scheduled_deadlines.insert(handle_id, deadline_tick);
      },
      | DeterministicEvent::Fired { handle_id, fired_tick, .. } => {
        completed = completed.saturating_add(1);
        if let Some(deadline_tick) = scheduled_deadlines.get(&handle_id) {
          let drift = fired_tick.saturating_sub(*deadline_tick);
          let budget = cmp::max(*deadline_tick, 1);
          let pct = (drift * 100) / budget;
          max_drift_pct = cmp::max(max_drift_pct, pct);
        }
      },
      | DeterministicEvent::Cancelled { .. } => {},
    }
  }

  StressReport { completed, max_drift_pct }
}

#[test]
fn stress_scheduler_handles_1000_jobs_without_drift() {
  let report = run_stress_profile(1_000, 0);
  assert_eq!(report.completed, 1_000);
  assert!(report.max_drift_pct <= 5, "drift pct {} exceeded budget", report.max_drift_pct);
}

#[test]
fn stress_scheduler_handles_10000_jobs_without_drift() {
  let report = run_stress_profile(10_000, 0);
  assert_eq!(report.completed, 10_000);
  assert!(report.max_drift_pct <= 5, "drift pct {} exceeded budget", report.max_drift_pct);
}

fn assert_deterministic_invariants(events: &[DeterministicEvent]) {
  #[derive(Default)]
  struct Record {
    scheduled_tick: u64,
    deadline_tick:  u64,
    cancelled_tick: Option<u64>,
    fired_ticks:    Vec<u64>,
  }

  let mut map: HashMap<u64, Record> = HashMap::new();
  for event in events {
    match *event {
      | DeterministicEvent::Scheduled { handle_id, scheduled_tick, deadline_tick } => {
        let record = map.entry(handle_id).or_default();
        record.scheduled_tick = scheduled_tick;
        record.deadline_tick = deadline_tick;
      },
      | DeterministicEvent::Fired { handle_id, fired_tick, .. } => {
        let record = map.entry(handle_id).or_default();
        record.fired_ticks.push(fired_tick);
        assert!(fired_tick >= record.scheduled_tick, "fire before schedule for handle {handle_id}");
        if record.deadline_tick > 0 {
          assert!(fired_tick >= record.deadline_tick, "fire before deadline for handle {handle_id}");
        }
        if let Some(cancelled_tick) = record.cancelled_tick {
          assert!(fired_tick <= cancelled_tick, "fire after cancellation for handle {handle_id}");
        }
      },
      | DeterministicEvent::Cancelled { handle_id, cancelled_tick } => {
        let record = map.entry(handle_id).or_default();
        if let Some(existing) = record.cancelled_tick {
          assert!(cancelled_tick >= existing, "cancelled tick regress for handle {handle_id}");
        }
        record.cancelled_tick = Some(cancelled_tick);
      },
    }
  }
}

#[test]
fn deterministic_log_records_schedule_fire_cancel() {
  let mut scheduler = build_scheduler();
  scheduler.enable_deterministic_log(16);

  let cancel_handle = scheduler.schedule_once(Duration::from_millis(1), SchedulerCommand::Noop).expect("cancel handle");
  assert!(scheduler.cancel(&cancel_handle));

  let fire_handle = scheduler.schedule_once(Duration::from_millis(2), SchedulerCommand::Noop).expect("fire handle");
  scheduler.run_for_test(4);

  let events = scheduler.diagnostics().deterministic_log();
  assert!(events.iter().any(
    |event| matches!(event, DeterministicEvent::Scheduled { handle_id, .. } if *handle_id == cancel_handle.raw())
  ));
  assert!(events.iter().any(
    |event| matches!(event, DeterministicEvent::Cancelled { handle_id, .. } if *handle_id == cancel_handle.raw())
  ));
  assert!(
    events
      .iter()
      .any(|event| matches!(event, DeterministicEvent::Fired { handle_id, .. } if *handle_id == fire_handle.raw()))
  );
}

#[test]
fn shutdown_executes_task_run_on_close_in_priority_order() {
  let mut scheduler = build_scheduler();
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  scheduler.register_on_close(RecordingTask::succeed(log.clone(), "user"), TaskRunPriority::User).expect("user");
  scheduler
    .register_on_close(RecordingTask::succeed(log.clone(), "runtime"), TaskRunPriority::Runtime)
    .expect("runtime");
  scheduler
    .register_on_close(RecordingTask::succeed(log.clone(), "system"), TaskRunPriority::SystemCritical)
    .expect("system");

  let summary = scheduler.shutdown();
  let observed = log.lock().clone();
  assert_eq!(observed, vec!["system", "runtime", "user"]);
  assert_eq!(summary.executed_tasks, 3);
  assert_eq!(summary.failed_tasks, 0);
}

#[test]
fn task_run_capacity_limits_registrations() {
  let toolbox = NoStdToolbox::default();
  let profile = SchedulerCapacityProfile::standard();
  let config = SchedulerConfig::new(Duration::from_millis(1), profile).with_task_run_capacity(1);
  let mut scheduler = Scheduler::new(toolbox, config);
  let task = RecordingTask::succeed(ArcShared::new(NoStdMutex::new(Vec::new())), "only");
  scheduler.register_on_close(task.clone(), TaskRunPriority::User).expect("first");
  let err = scheduler.register_on_close(task, TaskRunPriority::User).expect_err("capacity");
  assert_eq!(err, SchedulerError::TaskRunCapacityExceeded);
}

#[test]
fn shutdown_reports_failed_tasks() {
  let mut scheduler = build_scheduler();
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  scheduler.register_on_close(RecordingTask::fail(log.clone(), "boom"), TaskRunPriority::Runtime).expect("fail");
  let summary = scheduler.shutdown();
  assert_eq!(summary.executed_tasks, 0);
  assert_eq!(summary.failed_tasks, 1);
  assert!(scheduler.warnings().iter().any(|warning| matches!(warning, SchedulerWarning::TaskRunFailed { .. })));
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
