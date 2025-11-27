extern crate std;

use alloc::boxed::Box;
#[allow(clippy::disallowed_types)]
use std::{sync::Mutex, time::Duration};

use fraktor_utils_rs::{
  core::{runtime_toolbox::SyncMutexFamily, sync::ArcShared, time::TimerInstant},
  std::runtime_toolbox::{StdMutexFamily, StdToolbox},
};
use tokio::{
  runtime::Handle,
  task::JoinHandle,
  time::{MissedTickBehavior, interval},
};

use crate::core::{
  event_stream::{EventStreamEvent, EventStreamGeneric},
  scheduler::{
    SchedulerTickMetricsProbe, TickDriver, TickDriverControl, TickDriverError, TickDriverFactory,
    TickDriverHandleGeneric, TickDriverId, TickDriverKind, TickFeedHandle, next_tick_driver_id,
  },
};

#[derive(Clone)]
struct StdMetricsOptions {
  event_stream: ArcShared<EventStreamGeneric<StdToolbox>>,
  interval:     Duration,
}

/// Factory producing Tokio interval-based drivers.
pub(crate) struct TokioIntervalDriverFactory {
  handle:     Handle,
  resolution: Duration,
  metrics:    Option<StdMetricsOptions>,
}

impl TokioIntervalDriverFactory {
  pub(crate) const fn new(handle: Handle, resolution: Duration) -> Self {
    Self { handle, resolution, metrics: None }
  }

  pub(crate) fn with_metrics(
    mut self,
    event_stream: ArcShared<EventStreamGeneric<StdToolbox>>,
    interval: Duration,
  ) -> Self {
    self.metrics = Some(StdMetricsOptions { event_stream, interval });
    self
  }
}

impl TickDriverFactory<StdToolbox> for TokioIntervalDriverFactory {
  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn resolution(&self) -> Duration {
    self.resolution
  }

  fn build(&self) -> Result<Box<dyn TickDriver<StdToolbox>>, TickDriverError> {
    Ok(Box::new(TokioIntervalDriver {
      id:         next_tick_driver_id(),
      handle:     self.handle.clone(),
      resolution: self.resolution,
      metrics:    self.metrics.clone(),
    }))
  }
}

struct TokioIntervalDriver {
  id:         TickDriverId,
  handle:     Handle,
  resolution: Duration,
  metrics:    Option<StdMetricsOptions>,
}

impl TickDriver<StdToolbox> for TokioIntervalDriver {
  fn id(&self) -> TickDriverId {
    self.id
  }

  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn resolution(&self) -> Duration {
    self.resolution
  }

  fn start(&self, feed: TickFeedHandle<StdToolbox>) -> Result<TickDriverHandleGeneric<StdToolbox>, TickDriverError> {
    let mut ticker = interval(self.resolution);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let handle_clone = self.handle.clone();
    let feed_for_driver = feed.clone();
    let join = self.handle.spawn(async move {
      let mut ticker = ticker;
      loop {
        ticker.tick().await;
        feed_for_driver.enqueue(1);
      }
    });
    let metrics = self.metrics.as_ref().map(|options| {
      StdTickMetricsEmitter::spawn(
        &handle_clone,
        feed.clone(),
        self.resolution,
        self.kind(),
        options.event_stream.clone(),
        options.interval,
      )
    });
    let control: Box<dyn TickDriverControl> = Box::new(TokioIntervalDriverControl::new(join, metrics));
    let control = ArcShared::new(<StdMutexFamily as SyncMutexFamily>::create(control));
    Ok(TickDriverHandleGeneric::new(self.id, self.kind(), self.resolution, control))
  }
}

#[allow(clippy::disallowed_types)]
struct TokioIntervalDriverControl {
  join:    Mutex<Option<JoinHandle<()>>>,
  metrics: Mutex<Option<StdTickMetricsEmitter>>,
}

impl TokioIntervalDriverControl {
  #[allow(clippy::disallowed_types)]
  const fn new(join: JoinHandle<()>, metrics: Option<StdTickMetricsEmitter>) -> Self {
    Self { join: Mutex::new(Some(join)), metrics: Mutex::new(metrics) }
  }
}

impl TickDriverControl for TokioIntervalDriverControl {
  fn shutdown(&mut self) {
    #[allow(clippy::expect_used)]
    if let Some(handle) = self.join.lock().expect("lock").take() {
      handle.abort();
    }
    #[allow(clippy::expect_used)]
    if let Some(mut emitter) = self.metrics.lock().expect("lock").take() {
      emitter.shutdown();
    }
  }
}

#[allow(clippy::disallowed_types)]
struct StdTickMetricsEmitter {
  join: Mutex<Option<JoinHandle<()>>>,
}

impl StdTickMetricsEmitter {
  #[allow(clippy::disallowed_types)]
  fn spawn(
    handle: &Handle,
    feed: TickFeedHandle<StdToolbox>,
    resolution: Duration,
    driver: TickDriverKind,
    event_stream: ArcShared<EventStreamGeneric<StdToolbox>>,
    metrics_interval: Duration,
  ) -> Self {
    let mut ticker = interval(metrics_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let probe = SchedulerTickMetricsProbe::new(feed, resolution, driver);
    let join = handle.spawn(async move {
      let mut elapsed_ticks = 0_u64;
      let ticks_per_interval = ticks_for_interval(metrics_interval, resolution);
      loop {
        ticker.tick().await;
        elapsed_ticks = elapsed_ticks.saturating_add(ticks_per_interval);
        let now = TimerInstant::from_ticks(elapsed_ticks, resolution);
        let metrics = probe.snapshot(now);
        event_stream.publish(&EventStreamEvent::SchedulerTick(metrics));
      }
    });
    Self { join: Mutex::new(Some(join)) }
  }

  fn shutdown(&mut self) {
    #[allow(clippy::expect_used)]
    if let Some(handle) = self.join.lock().expect("lock").take() {
      handle.abort();
    }
  }
}

fn ticks_for_interval(interval: Duration, resolution: Duration) -> u64 {
  let interval_nanos = interval.as_nanos();
  let resolution_nanos = resolution.as_nanos().max(1);
  let ticks = interval_nanos / resolution_nanos;
  if ticks == 0 { 1 } else { ticks as u64 }
}
