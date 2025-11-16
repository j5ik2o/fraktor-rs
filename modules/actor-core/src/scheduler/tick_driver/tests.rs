//! Tick driver bootstrap integration tests.

use alloc::{boxed::Box, vec, vec::Vec};
use core::{
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::{sync::{ArcShared, NoStdMutex, sync_mutex_like::SpinSyncMutex}, time::TimerInstant};

use crate::{
  NoStdToolbox,
  scheduler::{
    HardwareKind,
    ManualTestDriver,
    SchedulerConfig,
    SchedulerContext,
    SchedulerRunnable,
    SchedulerCommand,
    ExecutionBatch,
    TickDriver,
    TickDriverBootstrap,
    TickDriverConfig,
    TickDriverControl,
    TickDriverError,
    TickDriverFactory,
    TickDriverId,
    TickDriverKind, TickExecutorSignal, TickFeed, TickPulseHandler, TickPulseSource,
  },
};

struct TestDriverFactory {
  start_count: ArcShared<AtomicUsize>,
  stop_count: ArcShared<AtomicUsize>,
}

impl TestDriverFactory {
  fn shared() -> ArcShared<Self> {
    ArcShared::new(Self {
      start_count: ArcShared::new(AtomicUsize::new(0)),
      stop_count: ArcShared::new(AtomicUsize::new(0)),
    })
  }
}

impl TickDriverFactory<NoStdToolbox> for TestDriverFactory {
  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn resolution(&self) -> Duration {
    Duration::from_millis(10)
  }

  fn build(&self) -> Result<Box<dyn TickDriver<NoStdToolbox>>, TickDriverError> {
    Ok(Box::new(TestDriver {
      id: TickDriverId::new(1),
      start_count: self.start_count.clone(),
      stop_count: self.stop_count.clone(),
    }))
  }
}

struct TestDriver {
  id: TickDriverId,
  start_count: ArcShared<AtomicUsize>,
  stop_count: ArcShared<AtomicUsize>,
}

struct TestDriverControl {
  stop_count: ArcShared<AtomicUsize>,
}

impl TickDriverControl for TestDriverControl {
  fn shutdown(&self) {
    self.stop_count.fetch_add(1, Ordering::SeqCst);
  }
}

impl TickDriver<NoStdToolbox> for TestDriver {
  fn id(&self) -> TickDriverId {
    self.id
  }

  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn resolution(&self) -> Duration {
    Duration::from_millis(10)
  }

  fn start(
    &self,
    _feed: crate::scheduler::TickFeedHandle<NoStdToolbox>,
  ) -> Result<crate::scheduler::TickDriverHandle, TickDriverError> {
    self.start_count.fetch_add(1, Ordering::SeqCst);
    Ok(crate::scheduler::TickDriverHandle::new(
      self.id,
      self.kind(),
      self.resolution(),
      ArcShared::new(TestDriverControl { stop_count: self.stop_count.clone() }),
    ))
  }
}

#[test]
fn bootstrap_starts_and_stops_driver_via_factory() {
  let factory = TestDriverFactory::shared();
  let config = TickDriverConfig::auto_with_factory(factory.clone());
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");
  assert_eq!(factory.start_count.load(Ordering::SeqCst), 1);

  TickDriverBootstrap::shutdown(runtime.driver().clone());
  assert_eq!(factory.stop_count.load(Ordering::SeqCst), 1);
}

struct TestPulseSource {
  handler: SpinSyncMutex<Option<(unsafe extern "C" fn(*mut core::ffi::c_void), *mut core::ffi::c_void)>>,
  resolution: Duration,
}

impl TestPulseSource {
  const fn new(resolution: Duration) -> Self {
    Self { handler: SpinSyncMutex::new(None), resolution }
  }

  fn trigger(&self) {
    if let Some((func, ctx)) = *self.handler.lock() {
      unsafe { (func)(ctx); }
    }
  }

  fn reset(&self) {
    *self.handler.lock() = None;
  }
}

unsafe impl Send for TestPulseSource {}
unsafe impl Sync for TestPulseSource {}

impl TickPulseSource for TestPulseSource {
  fn enable(&self) -> Result<(), TickDriverError> {
    Ok(())
  }

  fn disable(&self) {}

  fn set_callback(&self, handler: TickPulseHandler) {
    *self.handler.lock() = Some((handler.func, handler.ctx));
  }

  fn resolution(&self) -> Duration {
    self.resolution
  }
}

static TEST_PULSE: TestPulseSource = TestPulseSource::new(Duration::from_millis(2));

#[test]
fn hardware_driver_enqueues_isr_pulses() {
  TEST_PULSE.reset();
  let config = TickDriverConfig::hardware(&TEST_PULSE);
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  TEST_PULSE.trigger();
  let resolution = ctx.scheduler().lock().config().resolution();
  let now = TimerInstant::from_ticks(1, resolution);
  let feed = runtime.feed().expect("feed");
  assert!(feed.driver_active());
  let metrics = feed.snapshot(now, TickDriverKind::Hardware { source: HardwareKind::Custom });
  assert_eq!(metrics.enqueued_total(), 1);

  TickDriverBootstrap::shutdown(runtime.driver().clone());
}

#[test]
fn enqueue_from_isr_preserves_order_and_metrics() {
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::<NoStdToolbox>::new(Duration::from_millis(1), 1, signal.clone());

  feed.enqueue_from_isr(1);
  feed.enqueue_from_isr(1);

  let mut drained = Vec::new();
  feed.drain_pending(|ticks| drained.push(ticks));
  assert_eq!(drained, vec![1]);
  assert!(feed.driver_active());

  let now = TimerInstant::from_ticks(1, Duration::from_millis(1));
  let metrics = feed.snapshot(now, TickDriverKind::Hardware { source: HardwareKind::Custom });
  assert_eq!(metrics.enqueued_total(), 1);
  assert_eq!(metrics.dropped_total(), 1);
  assert!(signal.arm(), "signal should observe pending work");
}

#[test]
fn hardware_driver_watchdog_marks_inactive_on_shutdown() {
  TEST_PULSE.reset();
  let config = TickDriverConfig::hardware(&TEST_PULSE);
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  TEST_PULSE.trigger();
  let feed = runtime.feed().expect("feed");
  assert!(feed.driver_active());

  TickDriverBootstrap::shutdown(runtime.driver().clone());
  assert!(!feed.driver_active());
}

struct ManualRunnable {
  log: ArcShared<NoStdMutex<Vec<&'static str>>>,
  label: &'static str,
}

impl SchedulerRunnable for ManualRunnable {
  fn run(&self, _batch: &ExecutionBatch) {
    self.log.lock().push(self.label);
  }
}

#[test]
fn manual_driver_runs_jobs_without_executor() {
  let driver = ManualTestDriver::<NoStdToolbox>::new();
  let config = TickDriverConfig::manual(driver);
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());

  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");
  assert!(runtime.feed().is_none());
  let controller = runtime.manual_controller().expect("manual controller");

  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let runnable: ArcShared<ManualRunnable> = ArcShared::new(ManualRunnable { log: log.clone(), label: "manual" });
  {
    let scheduler = ctx.scheduler();
    let mut guard = scheduler.lock();
    guard
      .schedule_once(Duration::from_millis(10), SchedulerCommand::RunRunnable { runnable, dispatcher: None })
      .expect("schedule");
  }

  controller.inject_ticks(1);
  controller.drive();

  assert_eq!(log.lock().len(), 1);
}
