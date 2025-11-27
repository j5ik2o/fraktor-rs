//! Tick driver bootstrap integration tests.

use alloc::{boxed::Box, vec, vec::Vec};
use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
  time::TimerInstant,
};

use crate::core::{
  event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber},
  logging::LogLevel,
  scheduler::{
    ExecutionBatch, HardwareKind, ManualTestDriver, Scheduler, SchedulerCommand, SchedulerConfig, SchedulerContext,
    SchedulerRunnable, SchedulerTickExecutor, TICK_DRIVER_MATRIX, TickDriverBootstrap, TickDriverConfig,
    TickDriverError, TickDriverKind, TickExecutorSignal, TickFeed, TickMetricsMode, TickPulseHandler, TickPulseSource,
  },
};

struct RecordingSubscriber {
  events: SpinSyncMutex<Vec<EventStreamEvent<NoStdToolbox>>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: SpinSyncMutex::new(Vec::new()) }
  }

  fn snapshot(&self) -> Vec<EventStreamEvent<NoStdToolbox>> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber<NoStdToolbox> for RecordingSubscriber {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

/// Raw handler state storing function pointer and context.
type RawHandlerState =
  ArcShared<SpinSyncMutex<Option<(unsafe extern "C" fn(*mut core::ffi::c_void), *mut core::ffi::c_void)>>>;

/// Wrapper for handler state that implements Send + Sync.
///
/// This is safe because the raw pointer is only used within interrupt context
/// callbacks and the mutex ensures exclusive access.
#[derive(Clone)]
struct TestPulseHandlerState(RawHandlerState);

unsafe impl Send for TestPulseHandlerState {}
unsafe impl Sync for TestPulseHandlerState {}

impl TestPulseHandlerState {
  fn new() -> Self {
    Self(ArcShared::new(SpinSyncMutex::new(None)))
  }

  fn lock(
    &self,
  ) -> impl core::ops::DerefMut<Target = Option<(unsafe extern "C" fn(*mut core::ffi::c_void), *mut core::ffi::c_void)>> + '_
  {
    self.0.lock()
  }
}

/// Test control handle for triggering and resetting pulse callbacks.
#[derive(Clone)]
struct TestPulseHandle {
  handler: TestPulseHandlerState,
}

impl TestPulseHandle {
  fn trigger(&self) {
    if let Some((func, ctx)) = *self.handler.lock() {
      unsafe {
        (func)(ctx);
      }
    }
  }

  fn reset(&self) {
    *self.handler.lock() = None;
  }
}

struct TestPulseSource {
  handler:    TestPulseHandlerState,
  resolution: Duration,
}

impl TestPulseSource {
  fn new(resolution: Duration, handler: TestPulseHandlerState) -> Self {
    Self { handler, resolution }
  }
}

impl TickPulseSource for TestPulseSource {
  fn enable(&mut self) -> Result<(), TickDriverError> {
    Ok(())
  }

  fn disable(&mut self) {}

  fn set_callback(&mut self, handler: TickPulseHandler) {
    *self.handler.lock() = Some((handler.func, handler.ctx));
  }

  fn resolution(&self) -> Duration {
    self.resolution
  }
}

fn spawn_test_handler() -> (TestPulseHandlerState, TestPulseHandle) {
  let handler = TestPulseHandlerState::new();
  let handle = TestPulseHandle { handler: handler.clone() };
  (handler, handle)
}

fn hardware_test_config(handler: TestPulseHandlerState, pulse_resolution: Duration) -> TickDriverConfig<NoStdToolbox> {
  TickDriverConfig::new(move |ctx| {
    use fraktor_utils_rs::core::{runtime_toolbox::ToolboxMutex, sync::ArcShared};

    use super::{HardwareKind, HardwareTickDriver, TickDriver, TickDriverRuntime, TickExecutorSignal, TickFeed};

    let scheduler: ArcShared<ToolboxMutex<Scheduler<NoStdToolbox>, NoStdToolbox>> = ctx.scheduler();
    let (resolution, capacity) = {
      let guard = scheduler.lock();
      let cfg = guard.config();
      (cfg.resolution(), cfg.profile().tick_buffer_quota())
    };

    let source = TestPulseSource::new(pulse_resolution, handler.clone());
    let mut driver = HardwareTickDriver::new(Box::new(source), HardwareKind::Custom);
    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, capacity, signal);
    let handle = driver.start(feed.clone())?;

    Ok(TickDriverRuntime::new(handle, feed))
  })
}

fn run_hardware_driver_enqueues_isr_pulses() {
  let (handler, handle) = spawn_test_handler();
  handle.reset();
  let config = hardware_test_config(handler, Duration::from_millis(2));
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  handle.trigger();
  let resolution = ctx.scheduler().lock().config().resolution();
  let now = TimerInstant::from_ticks(1, resolution);
  let feed = runtime.feed().expect("feed");
  assert!(feed.driver_active());
  let metrics = feed.snapshot(now, TickDriverKind::Hardware { source: HardwareKind::Custom });
  assert_eq!(metrics.enqueued_total(), 1);

  TickDriverBootstrap::shutdown(runtime.driver());
  handle.reset();
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

fn run_hardware_driver_watchdog_marks_inactive_on_shutdown() {
  let (handler, handle) = spawn_test_handler();
  handle.reset();
  let config = hardware_test_config(handler, Duration::from_millis(2));
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  handle.trigger();
  let feed = runtime.feed().expect("feed");
  assert!(feed.driver_active());

  TickDriverBootstrap::shutdown(runtime.driver());
  assert!(!feed.driver_active());
  handle.reset();
}

#[test]
fn hardware_driver_isr_bridge_behaviors() {
  run_hardware_driver_enqueues_isr_pulses();
  run_hardware_driver_watchdog_marks_inactive_on_shutdown();
}

struct ManualRunnable {
  log:   ArcShared<NoStdMutex<Vec<&'static str>>>,
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
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let ctx = SchedulerContext::new(NoStdToolbox::default(), scheduler_config);

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

#[test]
fn manual_driver_rejected_when_runner_api_disabled() {
  let driver = ManualTestDriver::<NoStdToolbox>::new();
  let config = TickDriverConfig::manual(driver);
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());

  let result = TickDriverBootstrap::provision(&config, &ctx);
  assert!(matches!(result, Err(TickDriverError::ManualDriverDisabled)));
}

#[test]
fn embedded_quickstart_template_runs_ticks() {
  let (handler, handle) = spawn_test_handler();
  handle.reset();
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let config = hardware_test_config(handler, Duration::from_millis(2));
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  let scheduler = ctx.scheduler();
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let runnable: ArcShared<ManualRunnable> = ArcShared::new(ManualRunnable { log: log.clone(), label: "embedded" });
  {
    let mut guard = scheduler.lock();
    guard
      .schedule_once(Duration::from_millis(2), SchedulerCommand::RunRunnable { runnable, dispatcher: None })
      .expect("schedule job");
  }

  let feed = runtime.feed().expect("feed").clone();
  let signal = feed.signal();
  let mut executor = SchedulerTickExecutor::new(scheduler.clone(), feed, signal);

  for _ in 0..4 {
    handle.trigger();
    executor.drive_pending();
  }

  assert_eq!(log.lock().as_slice(), &["embedded"]);

  TickDriverBootstrap::shutdown(runtime.driver());
}

#[test]
fn driver_matrix_lists_auto_and_hardware_entries() {
  let mut has_auto = false;
  let mut has_hardware = false;
  for entry in TICK_DRIVER_MATRIX {
    match entry.kind {
      | TickDriverKind::Auto => {
        has_auto = true;
        assert_eq!(entry.label, "auto-std");
        assert!(!entry.test_only);
      },
      | TickDriverKind::Hardware { .. } => {
        has_hardware = true;
        assert_eq!(entry.label, "hardware");
        assert!(!entry.test_only);
      },
      #[cfg(any(test, feature = "test-support"))]
      | TickDriverKind::ManualTest => {},
    }
  }
  assert!(has_auto, "auto entry missing");
  assert!(has_hardware, "hardware entry missing");
}

#[test]
fn driver_matrix_marks_manual_entry_as_test_only() {
  let manual = TICK_DRIVER_MATRIX.iter().find(|entry| entry.label == "manual-test");
  if let Some(entry) = manual {
    assert!(entry.test_only);
    assert!(matches!(entry.metrics_mode, TickMetricsMode::OnDemand));
  } else {
    panic!("manual entry missing in test build");
  }
}

#[test]
fn driver_metadata_records_driver_activation() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);
  let ctx = SchedulerContext::with_event_stream(NoStdToolbox::default(), SchedulerConfig::default(), event_stream);
  let (handler, handle) = spawn_test_handler();
  handle.reset();
  let config = hardware_test_config(handler, Duration::from_millis(2));

  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");
  let metadata = ctx.driver_metadata().expect("metadata");
  assert_eq!(metadata.driver_id, runtime.driver().id());

  let events = subscriber_impl.snapshot();
  assert!(
    events
      .iter()
      .any(|event| matches!(event, EventStreamEvent::TickDriver(snapshot) if snapshot.metadata.driver_id == runtime.driver().id())),
    "tick driver snapshot event not observed"
  );
}

#[test]
fn driver_snapshot_exposed_via_scheduler_context() {
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let (handler, handle) = spawn_test_handler();
  handle.reset();
  let config = hardware_test_config(handler, Duration::from_millis(2));

  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  let snapshot = ctx.driver_snapshot().expect("driver snapshot");
  assert_eq!(snapshot.metadata.driver_id, runtime.driver().id());
  assert_eq!(snapshot.kind, TickDriverKind::Hardware { source: HardwareKind::Custom });
  // Snapshot should reflect the driver's actual resolution, not scheduler's default
  assert_eq!(snapshot.resolution, Duration::from_millis(2));
  assert!(snapshot.auto.is_none());

  TickDriverBootstrap::shutdown(runtime.driver());
}

#[test]
fn manual_driver_disabled_emits_warning() {
  let event_stream = ArcShared::new(EventStreamGeneric::<NoStdToolbox>::default());
  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);
  let ctx = SchedulerContext::with_event_stream(NoStdToolbox::default(), SchedulerConfig::default(), event_stream);
  let config = TickDriverConfig::manual(ManualTestDriver::new());

  let result = TickDriverBootstrap::provision(&config, &ctx);
  assert!(matches!(result, Err(TickDriverError::ManualDriverDisabled)));

  let events = subscriber_impl.snapshot();
  assert!(
    events.iter().any(|event| matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Warn)),
    "warning log not observed"
  );
}
