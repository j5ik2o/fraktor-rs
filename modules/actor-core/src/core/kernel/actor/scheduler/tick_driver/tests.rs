//! Tick driver bootstrap integration tests.

use alloc::{boxed::Box, vec, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::core::{
  sync::{ArcShared, SpinSyncMutex},
  time::TimerInstant,
};

use super::{
  SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverId, TickDriverKind, TickDriverProvision,
  TickDriverProvisioningContext, TickDriverStopper, TickExecutorSignal, TickFeed, TickFeedHandle,
  bootstrap::TickDriverBootstrap,
};
use crate::core::kernel::{
  actor::scheduler::{SchedulerConfig, SchedulerContext},
  event::stream::{EventStreamEvent, EventStreamShared, EventStreamSubscriber, tests::subscriber_handle},
};

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

// シンプルなテスト用ドライバー: スレッドを使わずにインライン実行
struct InlineTestDriver {
  id:         TickDriverId,
  resolution: Duration,
}

impl InlineTestDriver {
  fn new(resolution: Duration) -> Self {
    Self { id: TickDriverId::new(42), resolution }
  }
}

struct InlineTestStopper;

impl TickDriverStopper for InlineTestStopper {
  fn stop(self: Box<Self>) {}
}

impl TickDriver for InlineTestDriver {
  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn provision(
    self: Box<Self>,
    feed: TickFeedHandle,
    mut executor: SchedulerTickExecutor,
  ) -> Result<TickDriverProvision, TickDriverError> {
    // インラインで1ティックを送る
    feed.enqueue(1);
    executor.drive_pending();
    Ok(TickDriverProvision {
      resolution:    self.resolution,
      id:            self.id,
      kind:          TickDriverKind::Auto,
      stopper:       Box::new(InlineTestStopper),
      auto_metadata: None,
    })
  }
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
  let metrics = feed.snapshot(now, TickDriverKind::Std);
  assert_eq!(metrics.enqueued_total(), 1);
  assert_eq!(metrics.dropped_total(), 1);
  assert!(signal.arm(), "signal should observe pending work");
}

#[test]
fn bootstrap_inline_driver_provisions_correctly() {
  let resolution = Duration::from_millis(3);
  let driver: Box<dyn TickDriver> = Box::new(InlineTestDriver::new(resolution));
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);

  let result = TickDriverBootstrap::provision(driver, &ctx).expect("provision");
  assert_eq!(result.bundle.kind(), TickDriverKind::Auto);
  assert_eq!(result.bundle.resolution(), resolution);
  assert!(result.bundle.feed().is_some(), "inline driver must provision a feed");
}

#[test]
fn driver_metadata_records_driver_activation() {
  let event_stream = EventStreamShared::default();
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = event_stream.subscribe(&subscriber);
  let scheduler_context = SchedulerContext::with_event_stream(SchedulerConfig::default(), event_stream);
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let driver: Box<dyn TickDriver> = Box::new(InlineTestDriver::new(Duration::from_millis(2)));

  let result = TickDriverBootstrap::provision(driver, &ctx).expect("provision");
  let snapshot = &result.snapshot;

  let events = events.lock().clone();
  assert!(
    events.iter().any(
      |event| matches!(event, EventStreamEvent::TickDriver(s) if s.metadata.driver_id == snapshot.metadata.driver_id)
    ),
    "tick driver snapshot event not observed"
  );
}

#[test]
fn driver_snapshot_exposed_via_provisioning() {
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let resolution = Duration::from_millis(2);
  let driver: Box<dyn TickDriver> = Box::new(InlineTestDriver::new(resolution));

  let result = TickDriverBootstrap::provision(driver, &ctx).expect("provision");
  let snapshot = &result.snapshot;

  assert_eq!(snapshot.kind, TickDriverKind::Auto);
  assert_eq!(snapshot.resolution, resolution);
  assert!(snapshot.auto.is_none());
}
