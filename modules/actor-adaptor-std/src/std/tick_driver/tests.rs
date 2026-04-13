extern crate std;

use alloc::{boxed::Box, vec::Vec};
use core::time::Duration;
use std::sync::Mutex;

use fraktor_actor_core_rs::core::kernel::{
  actor::scheduler::{
    SchedulerConfig, SchedulerContext,
    tick_driver::{
      AutoDriverMetadata, AutoProfileKind, SchedulerTickExecutor, SchedulerTickMetricsProbe, TickDriver,
      TickDriverBootstrap, TickDriverConfig as CoreTickDriverConfig, TickDriverControl, TickDriverControlShared,
      TickDriverError, TickDriverHandle, TickDriverId, TickDriverKind, TickDriverProvisioningContext, TickExecutorPump,
      TickFeedHandle, next_tick_driver_id,
    },
  },
  event::stream::{EventStreamEvent, EventStreamShared, EventStreamSubscriber, subscriber_handle_with_shared_factory},
};
use fraktor_utils_core_rs::core::{
  sync::{ArcShared, SharedAccess},
  time::TimerInstant,
};
use tokio::{
  runtime::Handle,
  task::JoinHandle,
  time::{MissedTickBehavior, interval},
};

use super::{default_tick_driver_config, tick_driver_config_with_resolution};

fn tokio_quickstart_with_event_stream(
  resolution: Duration,
  event_stream: EventStreamShared,
  metrics_interval: Duration,
) -> CoreTickDriverConfig {
  let feed_slot = ArcShared::new(Mutex::new(None));
  CoreTickDriverConfig::runtime(
    Box::new(TokioMetricsDriver::new(resolution, feed_slot.clone())),
    Box::new(TokioMetricsPump::new(resolution, event_stream, metrics_interval, feed_slot)),
  )
}

struct TokioMetricsDriver {
  id:         TickDriverId,
  resolution: Duration,
  feed_slot:  ArcShared<Mutex<Option<TickFeedHandle>>>,
}

impl TokioMetricsDriver {
  fn new(resolution: Duration, feed_slot: ArcShared<Mutex<Option<TickFeedHandle>>>) -> Self {
    Self { id: next_tick_driver_id(), resolution, feed_slot }
  }
}

impl TickDriver for TokioMetricsDriver {
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
    let handle = Handle::try_current().map_err(|_| TickDriverError::HandleUnavailable)?;
    if let Ok(mut slot) = self.feed_slot.lock() {
      *slot = Some(feed.clone());
    }
    let resolution = self.resolution;
    let tick_task = handle.spawn(async move {
      let mut ticker = interval(resolution);
      ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
      loop {
        ticker.tick().await;
        feed.enqueue(1);
      }
    });

    let control: Box<dyn TickDriverControl> = Box::new(TokioMetricsDriverControl { tick_task });
    let control = TickDriverControlShared::new(control);
    Ok(TickDriverHandle::new(self.id, TickDriverKind::Auto, resolution, control))
  }
}

struct TokioMetricsDriverControl {
  tick_task: JoinHandle<()>,
}

impl TickDriverControl for TokioMetricsDriverControl {
  fn shutdown(&self) {
    self.tick_task.abort();
  }
}

struct TokioMetricsPump {
  resolution:       Duration,
  event_stream:     EventStreamShared,
  metrics_interval: Duration,
  feed_slot:        ArcShared<Mutex<Option<TickFeedHandle>>>,
}

impl TokioMetricsPump {
  fn new(
    resolution: Duration,
    event_stream: EventStreamShared,
    metrics_interval: Duration,
    feed_slot: ArcShared<Mutex<Option<TickFeedHandle>>>,
  ) -> Self {
    Self { resolution, event_stream, metrics_interval, feed_slot }
  }
}

impl TickExecutorPump for TokioMetricsPump {
  fn spawn(&mut self, mut executor: SchedulerTickExecutor) -> Result<Box<dyn TickDriverControl>, TickDriverError> {
    let handle = Handle::try_current().map_err(|_| TickDriverError::HandleUnavailable)?;
    let metrics_feed =
      self.feed_slot.lock().ok().and_then(|guard| guard.clone()).ok_or(TickDriverError::DriverStopped)?;
    let resolution = self.resolution;
    let executor_task = handle.spawn(async move {
      loop {
        executor.drive_pending();
        tokio::time::sleep(resolution / 10).await;
      }
    });

    let metrics_interval = self.metrics_interval;
    let metrics_event_stream = self.event_stream.clone();
    let metrics_task = handle.spawn(async move {
      let probe = SchedulerTickMetricsProbe::new(metrics_feed, resolution, TickDriverKind::Auto);
      let mut ticker = interval(metrics_interval);
      ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
      let mut elapsed_ticks = 0_u64;
      let ticks_per_interval = {
        let interval_nanos = metrics_interval.as_nanos();
        let resolution_nanos = resolution.as_nanos().max(1);
        let ticks = interval_nanos / resolution_nanos;
        if ticks == 0 { 1 } else { ticks as u64 }
      };
      loop {
        ticker.tick().await;
        elapsed_ticks = elapsed_ticks.saturating_add(ticks_per_interval);
        let now = TimerInstant::from_ticks(elapsed_ticks, resolution);
        let metrics = probe.snapshot(now);
        metrics_event_stream.publish(&EventStreamEvent::SchedulerTick(metrics));
      }
    });

    Ok(Box::new(TokioMetricsPumpControl { executor_task, metrics_task: Some(metrics_task) }))
  }

  fn auto_metadata(&self, driver_id: TickDriverId, resolution: Duration) -> Option<AutoDriverMetadata> {
    Some(AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id, resolution })
  }
}

struct TokioMetricsPumpControl {
  executor_task: JoinHandle<()>,
  metrics_task:  Option<JoinHandle<()>>,
}

impl TickDriverControl for TokioMetricsPumpControl {
  fn shutdown(&self) {
    self.executor_task.abort();
    if let Some(task) = &self.metrics_task {
      task.abort();
    }
  }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn tokio_interval_driver_produces_ticks() {
  let config = tick_driver_config_with_resolution(Duration::from_millis(5));
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let (mut runtime, _) = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  tokio::time::advance(Duration::from_millis(20)).await;
  tokio::task::yield_now().await;

  let resolution = ctx.scheduler().with_read(|s| s.config().resolution());
  let now = TimerInstant::from_ticks(1, resolution);
  let metrics = runtime.feed().expect("feed").snapshot(now, TickDriverKind::Auto);
  assert!(metrics.enqueued_total() > 0);

  runtime.shutdown();
}

struct RecordingSubscriber {
  events: ArcShared<Mutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<Mutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().expect("lock").push(event.clone());
  }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn tokio_interval_driver_publishes_tick_metrics_events() {
  let event_stream = EventStreamShared::default();
  let events = ArcShared::new(Mutex::new(Vec::new()));

  let subscriber = subscriber_handle_with_shared_factory(RecordingSubscriber::new(events.clone()));
  let _subscription = event_stream.subscribe(&subscriber);

  let config =
    tokio_quickstart_with_event_stream(Duration::from_millis(5), event_stream.clone(), Duration::from_millis(50));
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let (mut runtime, _) = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  tokio::time::advance(Duration::from_millis(120)).await;
  tokio::task::yield_now().await;

  runtime.shutdown();

  let events = events.lock().expect("lock").clone();
  assert!(
    events
      .iter()
      .any(|event| matches!(event, EventStreamEvent::SchedulerTick(metrics) if metrics.enqueued_total() > 0))
  );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn default_config_provisions_driver() {
  let config = default_tick_driver_config();
  let scheduler_context = SchedulerContext::new(SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let (mut runtime, snapshot) = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");
  assert!(
    matches!(snapshot.auto.as_ref().map(|meta| meta.profile), Some(AutoProfileKind::Tokio)),
    "auto metadata must be recorded for tokio quickstart",
  );

  tokio::time::advance(Duration::from_millis(40)).await;
  tokio::task::yield_now().await;

  let resolution = ctx.scheduler().with_read(|s| s.config().resolution());
  let now = TimerInstant::from_ticks(1, resolution);
  let metrics = runtime.feed().expect("feed").snapshot(now, TickDriverKind::Auto);
  assert!(metrics.enqueued_total() > 0);

  runtime.shutdown();
}
