//! Tick driver bootstrap integration tests.

use alloc::{boxed::Box, vec, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::{
  sync::{ArcShared, NoStdMutex, sync_mutex_like::SpinSyncMutex},
  time::TimerInstant,
};

use crate::{
  NoStdToolbox,
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

struct TestPulseSource {
  handler:    SpinSyncMutex<Option<(unsafe extern "C" fn(*mut core::ffi::c_void), *mut core::ffi::c_void)>>,
  resolution: Duration,
}

impl TestPulseSource {
  const fn new(resolution: Duration) -> Self {
    Self { handler: SpinSyncMutex::new(None), resolution }
  }

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

fn spawn_test_pulse(resolution: Duration) -> &'static TestPulseSource {
  Box::leak(Box::new(TestPulseSource::new(resolution)))
}

fn hardware_test_config(pulse: &'static dyn TickPulseSource) -> TickDriverConfig<NoStdToolbox> {
  TickDriverConfig::new(move |ctx| {
    use fraktor_utils_core_rs::sync::ArcShared;

    use super::{HardwareKind, HardwareTickDriver, TickDriver, TickDriverRuntime, TickExecutorSignal, TickFeed};
    use crate::{NoStdToolbox, ToolboxMutex};

    let scheduler: ArcShared<ToolboxMutex<Scheduler<NoStdToolbox>, NoStdToolbox>> = ctx.scheduler();
    let (resolution, capacity) = {
      let guard = scheduler.lock();
      let cfg = guard.config();
      (cfg.resolution(), cfg.profile().tick_buffer_quota())
    };

    let driver = HardwareTickDriver::new(pulse, HardwareKind::Custom);
    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, capacity, signal);
    let handle = driver.start(feed.clone())?;

    Ok(TickDriverRuntime::new(handle, feed))
  })
}

fn run_hardware_driver_enqueues_isr_pulses() {
  let pulse = spawn_test_pulse(Duration::from_millis(2));
  pulse.reset();
  let config = hardware_test_config(pulse);
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  pulse.trigger();
  let resolution = ctx.scheduler().lock().config().resolution();
  let now = TimerInstant::from_ticks(1, resolution);
  let feed = runtime.feed().expect("feed");
  assert!(feed.driver_active());
  let metrics = feed.snapshot(now, TickDriverKind::Hardware { source: HardwareKind::Custom });
  assert_eq!(metrics.enqueued_total(), 1);

  TickDriverBootstrap::shutdown(runtime.driver());
  pulse.reset();
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
  let pulse = spawn_test_pulse(Duration::from_millis(2));
  pulse.reset();
  let config = hardware_test_config(pulse);
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  pulse.trigger();
  let feed = runtime.feed().expect("feed");
  assert!(feed.driver_active());

  TickDriverBootstrap::shutdown(runtime.driver());
  assert!(!feed.driver_active());
  pulse.reset();
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
  let pulse = spawn_test_pulse(Duration::from_millis(2));
  pulse.reset();
  let ctx = SchedulerContext::new(NoStdToolbox::default(), SchedulerConfig::default());
  let config = hardware_test_config(pulse);
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
    pulse.trigger();
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
  let pulse = spawn_test_pulse(Duration::from_millis(2));
  pulse.reset();
  let config = hardware_test_config(pulse);

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
  let pulse = spawn_test_pulse(Duration::from_millis(2));
  pulse.reset();
  let config = hardware_test_config(pulse);

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
