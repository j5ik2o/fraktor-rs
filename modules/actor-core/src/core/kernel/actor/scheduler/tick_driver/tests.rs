//! Tick driver bootstrap integration tests.

use alloc::{boxed::Box, vec, vec::Vec};
use core::{
  sync::atomic::{AtomicBool, AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_rs::core::{
  sync::{ArcShared, NoStdMutex, SharedAccess, SpinSyncMutex},
  time::TimerInstant,
};

use super::bootstrap::TickDriverBootstrap;
use crate::core::kernel::{
  actor::scheduler::{
    ExecutionBatch, SchedulerCommand, SchedulerConfig, SchedulerContext, SchedulerRunnable,
    tick_driver::{
      AutoDriverMetadata, AutoProfileKind, HardwareKind, HardwareTickDriver, ManualTestDriver, SchedulerTickExecutor,
      TickDriver, TickDriverConfig, TickDriverControl, TickDriverError, TickDriverHandle, TickDriverId, TickDriverKind,
      TickDriverProvisioningContext, TickExecutorPump, TickExecutorSignal, TickFeed, TickFeedHandle, TickPulseHandler,
      TickPulseSource,
    },
  },
  event::{
    logging::LogLevel,
    stream::{EventStreamEvent, EventStreamShared, EventStreamSubscriber, subscriber_handle},
  },
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum TickMetricsMode {
  AutoPublish {
    interval: Duration,
  },
  #[default]
  OnDemand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TickDriverGuideEntry {
  kind:               TickDriverKind,
  label:              &'static str,
  description:        &'static str,
  default_resolution: Duration,
  metrics_mode:       TickMetricsMode,
  test_only:          bool,
}

impl TickDriverGuideEntry {
  const fn new(
    kind: TickDriverKind,
    label: &'static str,
    description: &'static str,
    default_resolution: Duration,
    metrics_mode: TickMetricsMode,
    test_only: bool,
  ) -> Self {
    Self { kind, label, description, default_resolution, metrics_mode, test_only }
  }

  const fn auto() -> Self {
    Self::new(
      TickDriverKind::Auto,
      "auto-std",
      "Tokio locator (StdTickDriverConfig::tokio_quickstart)",
      Duration::from_millis(10),
      TickMetricsMode::AutoPublish { interval: Duration::from_secs(1) },
      false,
    )
  }

  const fn hardware() -> Self {
    Self::new(
      TickDriverKind::Hardware { source: HardwareKind::Custom },
      "hardware",
      "TickPulseSource attachment for no_std targets",
      Duration::from_millis(1),
      TickMetricsMode::AutoPublish { interval: Duration::from_secs(1) },
      false,
    )
  }

  const fn manual() -> Self {
    Self::new(
      TickDriverKind::ManualTest,
      "manual-test",
      "Runner API (ManualTestDriver) for deterministic tests",
      Duration::from_millis(10),
      TickMetricsMode::OnDemand,
      true,
    )
  }
}

const TICK_DRIVER_MATRIX: &[TickDriverGuideEntry] =
  &[TickDriverGuideEntry::auto(), TickDriverGuideEntry::hardware(), TickDriverGuideEntry::manual()];

struct RecordingSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
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

struct NoopTickDriverControl;

impl TickDriverControl for NoopTickDriverControl {
  fn shutdown(&self) {}
}

struct RuntimeTestDriver {
  id:           TickDriverId,
  resolution:   Duration,
  started_feed: ArcShared<SpinSyncMutex<Option<TickFeedHandle>>>,
}

impl RuntimeTestDriver {
  fn new(resolution: Duration, started_feed: ArcShared<SpinSyncMutex<Option<TickFeedHandle>>>) -> Self {
    Self { id: TickDriverId::new(77), resolution, started_feed }
  }
}

impl TickDriver for RuntimeTestDriver {
  fn id(&self) -> TickDriverId {
    self.id
  }

  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn resolution(&self) -> Duration {
    self.resolution
  }

  fn start(&mut self, feed: TickFeedHandle) -> Result<TickDriverHandle, TickDriverError> {
    *self.started_feed.lock() = Some(feed.clone());
    let control: Box<dyn TickDriverControl> = Box::new(NoopTickDriverControl);
    let control = ArcShared::new(NoStdMutex::new(control));
    Ok(TickDriverHandle::new(self.id, TickDriverKind::Auto, self.resolution, control))
  }
}

struct RuntimeTestPump {
  spawn_calls:    ArcShared<AtomicUsize>,
  shutdown_calls: ArcShared<AtomicUsize>,
}

impl RuntimeTestPump {
  fn new(spawn_calls: ArcShared<AtomicUsize>, shutdown_calls: ArcShared<AtomicUsize>) -> Self {
    Self { spawn_calls, shutdown_calls }
  }
}

impl TickExecutorPump for RuntimeTestPump {
  fn spawn(&mut self, mut executor: SchedulerTickExecutor) -> Result<Box<dyn TickDriverControl>, TickDriverError> {
    self.spawn_calls.fetch_add(1, Ordering::SeqCst);
    executor.drive_pending();
    Ok(Box::new(RuntimePumpControl::new(self.shutdown_calls.clone())))
  }

  fn auto_metadata(&self, driver_id: TickDriverId, resolution: Duration) -> Option<AutoDriverMetadata> {
    Some(AutoDriverMetadata { profile: AutoProfileKind::Custom, driver_id, resolution })
  }
}

struct RuntimePumpControl {
  shutdown_calls: ArcShared<AtomicUsize>,
  did_shutdown:   AtomicBool,
}

impl RuntimePumpControl {
  fn new(shutdown_calls: ArcShared<AtomicUsize>) -> Self {
    Self { shutdown_calls, did_shutdown: AtomicBool::new(false) }
  }
}

impl TickDriverControl for RuntimePumpControl {
  fn shutdown(&self) {
    if !self.did_shutdown.swap(true, Ordering::SeqCst) {
      self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
    }
  }
}

struct HardwareTestPump;

impl TickExecutorPump for HardwareTestPump {
  fn spawn(&mut self, mut executor: SchedulerTickExecutor) -> Result<Box<dyn TickDriverControl>, TickDriverError> {
    executor.drive_pending();
    Ok(Box::new(NoopTickDriverControl))
  }
}

fn spawn_test_handler() -> (TestPulseHandlerState, TestPulseHandle) {
  let handler = TestPulseHandlerState::new();
  let handle = TestPulseHandle { handler: handler.clone() };
  (handler, handle)
}

fn hardware_test_config(handler: TestPulseHandlerState, pulse_resolution: Duration) -> TickDriverConfig {
  TickDriverConfig::runtime(
    Box::new(HardwareTickDriver::new(Box::new(TestPulseSource::new(pulse_resolution, handler)), HardwareKind::Custom)),
    Box::new(HardwareTestPump),
  )
}

fn runtime_test_config(
  resolution: Duration,
  started_feed: ArcShared<SpinSyncMutex<Option<TickFeedHandle>>>,
  spawn_calls: ArcShared<AtomicUsize>,
  shutdown_calls: ArcShared<AtomicUsize>,
) -> TickDriverConfig {
  TickDriverConfig::runtime(
    Box::new(RuntimeTestDriver::new(resolution, started_feed)),
    Box::new(RuntimeTestPump::new(spawn_calls, shutdown_calls)),
  )
}

fn run_hardware_driver_enqueues_isr_pulses() {
  let (handler, handle) = spawn_test_handler();
  handle.reset();
  let config = hardware_test_config(handler, Duration::from_millis(2));
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let (mut bundle, _) = TickDriverBootstrap::provision(&config, &ctx).expect("bundle");

  handle.trigger();
  let resolution = ctx.scheduler().with_read(|s| s.config().resolution());
  let now = TimerInstant::from_ticks(1, resolution);
  let feed = bundle.feed().expect("feed");
  assert!(feed.driver_active());
  let metrics = feed.snapshot(now, TickDriverKind::Hardware { source: HardwareKind::Custom });
  assert_eq!(metrics.enqueued_total(), 1);

  bundle.shutdown();
  handle.reset();
}

#[test]
fn enqueue_from_isr_preserves_order_and_metrics() {
  let signal = TickExecutorSignal::new();
  let feed = TickFeed::new(Duration::from_millis(1), 1, signal.clone());

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
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let (mut bundle, _) = TickDriverBootstrap::provision(&config, &ctx).expect("bundle");

  handle.trigger();
  let feed = bundle.feed().expect("feed").clone();
  assert!(feed.driver_active());

  bundle.shutdown();
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
  let driver = ManualTestDriver::new();
  let config = TickDriverConfig::manual(driver);
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let scheduler_context = SchedulerContext::new(scheduler_config);
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);

  let (bundle, _) = TickDriverBootstrap::provision(&config, &ctx).expect("bundle");
  assert!(bundle.feed().is_none());
  let controller = bundle.manual_controller().expect("manual controller");

  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let runnable: ArcShared<ManualRunnable> = ArcShared::new(ManualRunnable { log: log.clone(), label: "manual" });
  ctx.scheduler().with_write(|s| {
    s.schedule_once(Duration::from_millis(10), SchedulerCommand::RunRunnable { runnable }).expect("schedule");
  });

  controller.inject_ticks(1);
  controller.drive();

  assert_eq!(log.lock().len(), 1);
}

#[test]
fn manual_driver_rejected_when_runner_api_disabled() {
  let driver = ManualTestDriver::new();
  let config = TickDriverConfig::manual(driver);
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);

  let result = TickDriverBootstrap::provision(&config, &ctx);
  assert!(matches!(result, Err(TickDriverError::ManualDriverDisabled)));
}

#[test]
fn embedded_quickstart_template_runs_ticks() {
  let (handler, handle) = spawn_test_handler();
  handle.reset();
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let config = hardware_test_config(handler, Duration::from_millis(2));
  let (mut bundle, _) = TickDriverBootstrap::provision(&config, &ctx).expect("bundle");

  let scheduler = ctx.scheduler();
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let runnable: ArcShared<ManualRunnable> = ArcShared::new(ManualRunnable { log: log.clone(), label: "embedded" });
  scheduler.with_write(|s| {
    s.schedule_once(Duration::from_millis(2), SchedulerCommand::RunRunnable { runnable }).expect("schedule job");
  });

  let feed = bundle.feed().expect("feed").clone();
  let signal = feed.signal();
  let mut executor = SchedulerTickExecutor::new(scheduler.clone(), feed, signal);

  for _ in 0..4 {
    handle.trigger();
    executor.drive_pending();
  }

  assert_eq!(log.lock().as_slice(), &["embedded"]);

  bundle.shutdown();
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
  let event_stream = EventStreamShared::default();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = event_stream.subscribe(&subscriber);
  let scheduler_context = SchedulerContext::with_event_stream(SchedulerConfig::default(), event_stream);
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let (handler, handle) = spawn_test_handler();
  handle.reset();
  let config = hardware_test_config(handler, Duration::from_millis(2));

  let (bundle, snapshot) = TickDriverBootstrap::provision(&config, &ctx).expect("bundle");
  assert_eq!(snapshot.metadata.driver_id, bundle.driver().id());

  let events = events.lock().clone();
  assert!(
    events
      .iter()
      .any(|event| matches!(event, EventStreamEvent::TickDriver(snapshot) if snapshot.metadata.driver_id == bundle.driver().id())),
    "tick driver snapshot event not observed"
  );
}

#[test]
fn driver_snapshot_exposed_via_provisioning() {
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let (handler, handle) = spawn_test_handler();
  handle.reset();
  let config = hardware_test_config(handler, Duration::from_millis(2));

  let (mut bundle, snapshot) = TickDriverBootstrap::provision(&config, &ctx).expect("bundle");

  assert_eq!(snapshot.metadata.driver_id, bundle.driver().id());
  assert_eq!(snapshot.kind, TickDriverKind::Hardware { source: HardwareKind::Custom });
  // Snapshot should reflect the driver's actual resolution, not scheduler's default
  assert_eq!(snapshot.resolution, Duration::from_millis(2));
  assert!(snapshot.auto.is_none());

  bundle.shutdown();
}

#[test]
fn manual_driver_disabled_emits_warning() {
  let event_stream = EventStreamShared::default();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = event_stream.subscribe(&subscriber);
  let scheduler_context = SchedulerContext::with_event_stream(SchedulerConfig::default(), event_stream);
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let config = TickDriverConfig::manual(ManualTestDriver::new());

  let result = TickDriverBootstrap::provision(&config, &ctx);
  assert!(matches!(result, Err(TickDriverError::ManualDriverDisabled)));

  let events = events.lock().clone();
  assert!(
    events.iter().any(|event| matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Warn)),
    "warning log not observed"
  );
}

#[test]
fn bootstrap_runtime_wiring_path_builds_core_tick_components() {
  let started_feed = ArcShared::new(SpinSyncMutex::new(None));
  let spawn_calls = ArcShared::new(AtomicUsize::new(0));
  let shutdown_calls = ArcShared::new(AtomicUsize::new(0));
  let resolution = Duration::from_millis(3);
  let config = runtime_test_config(resolution, started_feed.clone(), spawn_calls.clone(), shutdown_calls.clone());
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);

  let (mut bundle, snapshot) = TickDriverBootstrap::provision(&config, &ctx).expect("runtime bundle");

  assert_eq!(spawn_calls.load(Ordering::SeqCst), 1);
  assert_eq!(snapshot.kind, TickDriverKind::Auto);
  assert_eq!(snapshot.resolution, resolution);
  assert_eq!(snapshot.auto.as_ref().map(|metadata| metadata.profile), Some(AutoProfileKind::Custom));
  assert!(bundle.feed().is_some(), "runtime path must provision a core feed");

  bundle.shutdown();
  assert_eq!(shutdown_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn bootstrap_runtime_wiring_path_starts_driver_with_core_feed() {
  let started_feed = ArcShared::new(SpinSyncMutex::new(None));
  let spawn_calls = ArcShared::new(AtomicUsize::new(0));
  let shutdown_calls = ArcShared::new(AtomicUsize::new(0));
  let config = runtime_test_config(Duration::from_millis(5), started_feed.clone(), spawn_calls, shutdown_calls);
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);

  let (bundle, _) = TickDriverBootstrap::provision(&config, &ctx).expect("runtime bundle");
  let captured_feed = started_feed.lock().clone().expect("driver feed");
  let bundle_feed = bundle.feed().expect("bundle feed");

  assert!(
    ArcShared::ptr_eq(&captured_feed, bundle_feed),
    "TickDriverBootstrap は Runtime 経路で core 側の TickFeed を driver に渡す必要があります"
  );
}
