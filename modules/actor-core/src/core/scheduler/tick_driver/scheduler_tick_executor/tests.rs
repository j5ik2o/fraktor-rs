//! Scheduler tick executor tests.

use alloc::vec::Vec;
use core::time::Duration;

use fraktor_utils_core_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use crate::core::scheduler::{
  SchedulerCommand, SchedulerConfig, SchedulerContext, SchedulerRunnable, SchedulerTickExecutor, TickExecutorSignal,
  TickFeed,
};

#[derive(Clone)]
struct RecordingRunnable {
  log:   ArcShared<NoStdMutex<Vec<&'static str>>>,
  label: &'static str,
}

impl SchedulerRunnable for RecordingRunnable {
  fn run(&self, _batch: &crate::core::scheduler::ExecutionBatch) {
    self.log.lock().push(self.label);
  }
}

#[test]
fn drive_pending_executes_scheduled_job() {
  let toolbox = NoStdToolbox::default();
  let config = SchedulerConfig::default();
  let context = SchedulerContext::new(toolbox, config);
  let scheduler = context.scheduler();
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::<NoStdToolbox>::new(config.resolution(), 8, signal.clone());
  let mut executor = SchedulerTickExecutor::new(scheduler.clone(), feed.clone(), signal);

  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let runnable = ArcShared::new(RecordingRunnable { log: log.clone(), label: "fired" });
  {
    let mut guard = scheduler.lock();
    guard
      .schedule_once(Duration::from_millis(10), SchedulerCommand::RunRunnable { runnable, dispatcher: None })
      .expect("schedule once");
  }

  feed.enqueue(1);
  executor.drive_pending();

  let entries = log.lock();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0], "fired");
}
