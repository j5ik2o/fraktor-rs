extern crate std;

use alloc::{boxed::Box, vec::Vec};
use core::time::Duration;
use std::sync::Mutex;

use fraktor_actor_rs::core::kernel::{
  actor::scheduler::{
    SchedulerConfig, SchedulerContext, SchedulerShared,
    tick_driver::{AutoProfileKind, TickDriverBootstrap, TickDriverKind, TickDriverProvisioningContext},
  },
  event::stream::{EventStreamEvent, EventStreamShared, EventStreamSubscriber, subscriber_handle},
};
use fraktor_utils_rs::core::{
  sync::{ArcShared, RuntimeMutex, SharedAccess},
  time::TimerInstant,
};
use tokio::time::{MissedTickBehavior, interval};

use crate::std::scheduler::TickDriverConfig;

fn tokio_quickstart_with_event_stream(
  resolution: Duration,
  event_stream: EventStreamShared,
  metrics_interval: Duration,
) -> fraktor_actor_rs::core::kernel::actor::scheduler::tick_driver::TickDriverConfig {
  use fraktor_actor_rs::core::kernel::actor::scheduler::tick_driver::{
    AutoDriverMetadata, AutoProfileKind, SchedulerTickExecutor, SchedulerTickMetricsProbe, TickDriverBundle,
    TickDriverControl, TickDriverHandle, TickExecutorSignal, TickFeed, next_tick_driver_id,
  };

  fraktor_actor_rs::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::new(move |ctx| {
    let handle = tokio::runtime::Handle::try_current().expect("Tokio runtime handle unavailable");

    let scheduler: SchedulerShared = ctx.scheduler();
    let capacity = scheduler.with_read(|s| s.config().profile().tick_buffer_quota());

    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, capacity, signal);
    let feed_clone = feed.clone();

    let tick_task = handle.spawn(async move {
      let mut ticker = interval(resolution);
      ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
      loop {
        ticker.tick().await;
        feed_clone.enqueue(1);
      }
    });

    let executor_feed = feed.clone();
    let executor_signal = executor_feed.signal();
    let executor_task = handle.spawn(async move {
      let mut executor = SchedulerTickExecutor::new(scheduler, executor_feed, executor_signal);
      loop {
        executor.drive_pending();
        tokio::time::sleep(resolution / 10).await;
      }
    });

    let metrics_feed = feed.clone();
    let metrics_event_stream = event_stream.clone();
    let probe = SchedulerTickMetricsProbe::new(metrics_feed, resolution, TickDriverKind::Auto);
    let metrics_task = handle.spawn(async move {
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

    struct TokioQuickstartControl {
      tick_task:     tokio::task::JoinHandle<()>,
      executor_task: tokio::task::JoinHandle<()>,
      metrics_task:  Option<tokio::task::JoinHandle<()>>,
    }

    impl TickDriverControl for TokioQuickstartControl {
      fn shutdown(&self) {
        self.tick_task.abort();
        self.executor_task.abort();
        if let Some(task) = &self.metrics_task {
          task.abort();
        }
      }
    }

    let driver_id = next_tick_driver_id();
    let control: Box<dyn TickDriverControl> =
      Box::new(TokioQuickstartControl { tick_task, executor_task, metrics_task: Some(metrics_task) });
    let control = ArcShared::new(RuntimeMutex::new(control));
    let driver_handle = TickDriverHandle::new(driver_id, TickDriverKind::Auto, resolution, control);
    let metadata = AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id, resolution };

    Ok(TickDriverBundle::new(driver_handle, feed).with_auto_metadata(metadata))
  })
}

#[tokio::test(flavor = "current_thread", start_paused = true)]

async fn tokio_interval_driver_produces_ticks() {
  let config = TickDriverConfig::with_resolution(Duration::from_millis(5));
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
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
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
  let config = TickDriverConfig::default_config();
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
