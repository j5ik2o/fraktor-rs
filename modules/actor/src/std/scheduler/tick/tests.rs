extern crate std;

use alloc::vec::Vec;
use core::time::Duration;
use std::sync::Mutex;

use fraktor_utils_rs::{
  core::{sync::ArcShared, time::TimerInstant},
  std::runtime_toolbox::StdToolbox,
};

use crate::{
  core::{
    event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber},
    scheduler::{AutoProfileKind, SchedulerConfig, SchedulerContext, TickDriverBootstrap, TickDriverKind},
  },
  std::scheduler::tick::TickDriverConfig,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::expect_used)]
async fn tokio_interval_driver_produces_ticks() {
  let config = TickDriverConfig::tokio_quickstart_with_resolution(Duration::from_millis(5));
  let ctx = SchedulerContext::new(StdToolbox::default(), SchedulerConfig::default());
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  tokio::time::sleep(Duration::from_millis(20)).await;
  let resolution = ctx.scheduler().lock().config().resolution();
  let now = TimerInstant::from_ticks(1, resolution);
  let metrics = runtime.feed().expect("feed").snapshot(now, TickDriverKind::Auto);
  assert!(metrics.enqueued_total() > 0);

  TickDriverBootstrap::shutdown(runtime.driver());
}

struct RecordingSubscriber {
  events: Mutex<Vec<EventStreamEvent<StdToolbox>>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: Mutex::new(Vec::new()) }
  }

  #[allow(clippy::expect_used)]
  fn snapshot(&self) -> Vec<EventStreamEvent<StdToolbox>> {
    self.events.lock().expect("lock").clone()
  }
}

impl EventStreamSubscriber<StdToolbox> for RecordingSubscriber {
  #[allow(clippy::expect_used)]
  fn on_event(&self, event: &EventStreamEvent<StdToolbox>) {
    self.events.lock().expect("lock").push(event.clone());
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::expect_used)]
async fn tokio_interval_driver_publishes_tick_metrics_events() {
  let event_stream = ArcShared::new(EventStreamGeneric::<StdToolbox>::default());
  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<StdToolbox>> = subscriber_impl.clone();
  let _subscription = EventStreamGeneric::subscribe_arc(&event_stream, &subscriber);

  let config = TickDriverConfig::tokio_quickstart_with_event_stream(
    Duration::from_millis(5),
    event_stream.clone(),
    Duration::from_millis(50),
  );
  let ctx = SchedulerContext::new(StdToolbox::default(), SchedulerConfig::default());
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  tokio::time::sleep(Duration::from_millis(120)).await;

  TickDriverBootstrap::shutdown(runtime.driver());

  let events = subscriber_impl.snapshot();
  assert!(
    events
      .iter()
      .any(|event| matches!(event, EventStreamEvent::SchedulerTick(metrics) if metrics.enqueued_total() > 0))
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::expect_used)]
async fn tokio_quickstart_helper_provisions_driver() {
  let config = TickDriverConfig::tokio_quickstart();
  let ctx = SchedulerContext::new(StdToolbox::default(), SchedulerConfig::default());
  let runtime = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  let snapshot = ctx.driver_snapshot().expect("snapshot");
  assert!(
    matches!(snapshot.auto.as_ref().map(|meta| meta.profile), Some(AutoProfileKind::Tokio)),
    "auto metadata must be recorded for tokio quickstart",
  );

  tokio::time::sleep(Duration::from_millis(40)).await;

  let resolution = ctx.scheduler().lock().config().resolution();
  let now = TimerInstant::from_ticks(1, resolution);
  let metrics = runtime.feed().expect("feed").snapshot(now, TickDriverKind::Auto);
  assert!(metrics.enqueued_total() > 0);

  TickDriverBootstrap::shutdown(runtime.driver());
}
