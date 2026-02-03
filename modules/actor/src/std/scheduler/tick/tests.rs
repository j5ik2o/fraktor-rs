extern crate std;

use alloc::vec::Vec;
use core::time::Duration;
use std::sync::Mutex;

use fraktor_utils_rs::{
  core::{
    sync::{ArcShared, SharedAccess},
    time::TimerInstant,
  },
  std::runtime_toolbox::StdToolbox,
};

use crate::{
  core::{
    event::stream::{EventStreamEvent, EventStreamSharedGeneric, EventStreamSubscriber, subscriber_handle},
    scheduler::{
      AutoProfileKind, SchedulerConfig, SchedulerContext, TickDriverKind, TickDriverProvisioningContext,
      tick_driver::TickDriverBootstrap,
    },
  },
  std::scheduler::tick::TickDriverConfig,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::expect_used)]
async fn tokio_interval_driver_produces_ticks() {
  let config = TickDriverConfig::tokio_quickstart_with_resolution(Duration::from_millis(5));
  let scheduler_context = SchedulerContext::new(StdToolbox::default(), SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let (mut runtime, _) = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  tokio::time::sleep(Duration::from_millis(20)).await;
  let resolution = ctx.scheduler().with_read(|s| s.config().resolution());
  let now = TimerInstant::from_ticks(1, resolution);
  let metrics = runtime.feed().expect("feed").snapshot(now, TickDriverKind::Auto);
  assert!(metrics.enqueued_total() > 0);

  runtime.shutdown();
}

struct RecordingSubscriber {
  events: ArcShared<Mutex<Vec<EventStreamEvent<StdToolbox>>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<Mutex<Vec<EventStreamEvent<StdToolbox>>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber<StdToolbox> for RecordingSubscriber {
  #[allow(clippy::expect_used)]
  fn on_event(&mut self, event: &EventStreamEvent<StdToolbox>) {
    self.events.lock().expect("lock").push(event.clone());
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::expect_used)]
async fn tokio_interval_driver_publishes_tick_metrics_events() {
  let event_stream = EventStreamSharedGeneric::<StdToolbox>::default();
  let events = ArcShared::new(Mutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = event_stream.subscribe(&subscriber);

  let config = TickDriverConfig::tokio_quickstart_with_event_stream(
    Duration::from_millis(5),
    event_stream.clone(),
    Duration::from_millis(50),
  );
  let scheduler_context = SchedulerContext::new(StdToolbox::default(), SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let (mut runtime, _) = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");

  tokio::time::sleep(Duration::from_millis(120)).await;

  runtime.shutdown();

  let events = events.lock().expect("lock").clone();
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
  let scheduler_context = SchedulerContext::new(StdToolbox::default(), SchedulerConfig::default());
  let ctx = TickDriverProvisioningContext::from_scheduler_context(&scheduler_context);
  let (mut runtime, snapshot) = TickDriverBootstrap::provision(&config, &ctx).expect("runtime");
  assert!(
    matches!(snapshot.auto.as_ref().map(|meta| meta.profile), Some(AutoProfileKind::Tokio)),
    "auto metadata must be recorded for tokio quickstart",
  );

  tokio::time::sleep(Duration::from_millis(40)).await;

  let resolution = ctx.scheduler().with_read(|s| s.config().resolution());
  let now = TimerInstant::from_ticks(1, resolution);
  let metrics = runtime.feed().expect("feed").snapshot(now, TickDriverKind::Auto);
  assert!(metrics.enqueued_total() > 0);

  runtime.shutdown();
}
