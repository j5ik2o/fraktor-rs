//! Scheduler tick executor tests.

use alloc::vec::Vec;
use core::time::Duration;

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use crate::actor::scheduler::{
  ExecutionBatch, SchedulerCommand, SchedulerConfig, SchedulerContext, SchedulerRunnable, SchedulerShared,
  tick_driver::{SchedulerTickExecutor, TickExecutorSignal, TickFeed},
};

#[derive(Clone)]
struct RecordingRunnable {
  log:   ArcShared<SpinSyncMutex<Vec<&'static str>>>,
  ticks: ArcShared<SpinSyncMutex<Vec<u64>>>,
  label: &'static str,
}

impl SchedulerRunnable for RecordingRunnable {
  fn run(&self, batch: &ExecutionBatch) {
    self.log.lock().push(self.label);
    self.ticks.lock().push(batch.execution_tick());
  }
}

struct TimeReadingRunnable {
  scheduler: SchedulerShared,
  times:     ArcShared<SpinSyncMutex<Vec<u64>>>,
}

impl SchedulerRunnable for TimeReadingRunnable {
  fn run(&self, _batch: &ExecutionBatch) {
    self.times.lock().push(self.scheduler.current_time_secs());
  }
}

#[test]
fn drive_pending_executes_scheduled_job() {
  let config = SchedulerConfig::default();
  let context = SchedulerContext::new(config);
  let scheduler = context.scheduler();
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::new(config.resolution(), 8, signal.clone());
  let mut executor = SchedulerTickExecutor::new(scheduler.clone(), feed.clone(), signal);

  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let ticks = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let runnable = ArcShared::new(RecordingRunnable { log: log.clone(), ticks: ticks.clone(), label: "fired" });
  scheduler.with_write(|s| {
    s.schedule_once(Duration::from_millis(10), SchedulerCommand::RunRunnable { runnable }).expect("schedule once");
  });

  feed.enqueue(1);
  executor.drive_pending();

  let entries = log.lock();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0], "fired");
  assert_eq!(*ticks.lock(), [1]);
}

#[test]
fn drive_pending_allows_runnable_to_read_current_time() {
  let config = SchedulerConfig::default();
  let context = SchedulerContext::new(config);
  let scheduler = context.scheduler();
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::new(config.resolution(), 8, signal.clone());
  let mut executor = SchedulerTickExecutor::new(scheduler.clone(), feed.clone(), signal);
  let times = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let runnable = ArcShared::new(TimeReadingRunnable { scheduler: scheduler.clone(), times: times.clone() });
  scheduler.with_write(|inner| {
    inner.schedule_once(config.resolution(), SchedulerCommand::RunRunnable { runnable }).expect("schedule once");
  });

  feed.enqueue(1);
  executor.drive_pending();

  assert_eq!(*times.lock(), [1]);
}
