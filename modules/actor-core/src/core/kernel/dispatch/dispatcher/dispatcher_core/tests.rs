use alloc::{boxed::Box, sync::Arc};
use core::{
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use super::DispatcherCore;
use crate::core::kernel::dispatch::dispatcher::{
  DispatcherConfig, ExecuteError, Executor, ExecutorShared, TrampolineState, shutdown_schedule::ShutdownSchedule,
};

struct StubExecutor {
  shutdowns: Arc<AtomicUsize>,
}

impl Executor for StubExecutor {
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    Ok(())
  }

  fn shutdown(&mut self) {
    self.shutdowns.fetch_add(1, Ordering::SeqCst);
  }
}

fn nz(value: usize) -> NonZeroUsize {
  NonZeroUsize::new(value).expect("non-zero")
}

fn make_core() -> (DispatcherCore, Arc<AtomicUsize>) {
  let shutdowns = Arc::new(AtomicUsize::new(0));
  let executor =
    ExecutorShared::new(Box::new(StubExecutor { shutdowns: Arc::clone(&shutdowns) }), TrampolineState::new());
  let settings = DispatcherConfig::new("test", nz(5), Some(Duration::from_millis(10)), Duration::from_secs(1));
  (DispatcherCore::new(&settings, executor), shutdowns)
}

#[test]
fn new_copies_settings_into_fields() {
  let (core, _) = make_core();
  assert_eq!(core.id(), "test");
  assert_eq!(core.throughput(), nz(5));
  assert_eq!(core.throughput_deadline(), Some(Duration::from_millis(10)));
  assert_eq!(core.shutdown_timeout(), Duration::from_secs(1));
  assert_eq!(core.inhabitants(), 0);
  assert_eq!(core.shutdown_schedule(), ShutdownSchedule::Unscheduled);
}

#[test]
fn mark_attach_increments_inhabitants() {
  let (mut core, _) = make_core();
  core.mark_attach();
  core.mark_attach();
  assert_eq!(core.inhabitants(), 2);
}

#[test]
fn mark_attach_cancels_scheduled_shutdown() {
  let (mut core, _) = make_core();
  core.mark_attach();
  core.mark_detach();
  let after_detach = core.schedule_shutdown_if_sensible();
  assert_eq!(after_detach, ShutdownSchedule::Scheduled);
  core.mark_attach();
  assert_eq!(core.shutdown_schedule(), ShutdownSchedule::Rescheduled);
}

#[test]
fn mark_detach_decrements_inhabitants() {
  let (mut core, _) = make_core();
  core.mark_attach();
  core.mark_attach();
  core.mark_detach();
  assert_eq!(core.inhabitants(), 1);
}

#[test]
fn schedule_shutdown_if_sensible_only_when_zero_inhabitants() {
  let (mut core, _) = make_core();
  core.mark_attach();
  // 1 inhabitant remains; no scheduling occurs.
  assert_eq!(core.schedule_shutdown_if_sensible(), ShutdownSchedule::Unscheduled);
  core.mark_detach();
  assert_eq!(core.schedule_shutdown_if_sensible(), ShutdownSchedule::Scheduled);
  // Calling again moves Scheduled -> Rescheduled.
  assert_eq!(core.schedule_shutdown_if_sensible(), ShutdownSchedule::Rescheduled);
}

#[test]
fn shutdown_resets_schedule_and_calls_executor() {
  let (mut core, shutdowns) = make_core();
  core.mark_attach();
  core.mark_detach();
  core.schedule_shutdown_if_sensible();
  core.shutdown();
  assert_eq!(core.shutdown_schedule(), ShutdownSchedule::Unscheduled);
  assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}
