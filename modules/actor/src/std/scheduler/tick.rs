//! Tokio-based tick driver implementations for std runtimes.

extern crate std;

use std::time::Duration;

use fraktor_utils_rs::{
  core::{
    sync::{ArcShared, ArcShared as Arc},
    time::TimerInstant,
  },
  std::runtime_toolbox::StdToolbox,
};
use tokio::runtime::Handle;

use crate::{
  core::{
    event_stream::EventStreamGeneric,
    scheduler::{TickDriverConfig, TickDriverFactoryRef},
  },
  std::scheduler::tick::tokio_impl::TokioIntervalDriverFactory,
};

mod tokio_impl;

/// Config helpers for std tick drivers.
pub struct StdTickDriverConfig;

impl StdTickDriverConfig {
  /// Builds a factory using the current Tokio runtime handle.
  ///
  /// # Panics
  ///
  /// Panics if no Tokio runtime handle is available in the current context.
  #[must_use]
  pub fn tokio_auto(resolution: Duration) -> TickDriverFactoryRef<StdToolbox> {
    let handle = Handle::try_current().expect("Tokio runtime handle unavailable");
    Self::tokio_with_handle(handle, resolution)
  }

  /// Creates a ready-to-use tick driver configuration for Tokio quickstart flows.
  #[must_use]
  pub fn tokio_quickstart() -> TickDriverConfig<StdToolbox> {
    Self::tokio_quickstart_with_resolution(Duration::from_millis(10))
  }

  /// Creates a Tokio quickstart configuration with custom resolution.
  ///
  /// This creates a complete tick driver setup including both the tick generator
  /// and the scheduler executor, similar to no_std hardware tick driver patterns.
  #[must_use]
  pub fn tokio_quickstart_with_resolution(resolution: Duration) -> TickDriverConfig<StdToolbox> {
    use fraktor_utils_rs::core::{runtime_toolbox::ToolboxMutex, sync::ArcShared};
    use tokio::time::{MissedTickBehavior, interval};

    use crate::core::scheduler::{
      AutoDriverMetadata, AutoProfileKind, Scheduler, SchedulerTickExecutor, TickDriverControl, TickDriverHandle,
      TickDriverKind, TickDriverRuntime, TickExecutorSignal, TickFeed, next_tick_driver_id,
    };

    TickDriverConfig::new(move |ctx| {
      let handle = Handle::try_current().expect("Tokio runtime handle unavailable");

      // Get scheduler, resolution, and capacity from context
      let scheduler: ArcShared<ToolboxMutex<Scheduler<StdToolbox>, StdToolbox>> = ctx.scheduler();
      let capacity = {
        let guard = scheduler.lock();
        guard.config().profile().tick_buffer_quota()
      };

      // Create tick driver components
      let signal = TickExecutorSignal::new();
      let feed = TickFeed::new(resolution, capacity, signal);
      let feed_clone = feed.clone();

      // Spawn tick generator task
      let tick_task = handle.spawn(async move {
        let mut ticker = interval(resolution);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
          ticker.tick().await;
          feed_clone.enqueue(1);
        }
      });

      // Spawn scheduler executor task
      let executor_feed = feed.clone();
      let executor_signal = executor_feed.signal();
      let executor_scheduler = scheduler.clone();
      let executor_task = handle.spawn(async move {
        let mut executor = SchedulerTickExecutor::new(executor_scheduler, executor_feed, executor_signal);
        loop {
          executor.drive_pending();
          tokio::time::sleep(resolution / 10).await;
        }
      });

      // Create driver handle
      struct TokioQuickstartControl {
        tick_task:     tokio::task::JoinHandle<()>,
        executor_task: tokio::task::JoinHandle<()>,
      }
      impl TickDriverControl for TokioQuickstartControl {
        fn shutdown(&self) {
          self.tick_task.abort();
          self.executor_task.abort();
        }
      }

      let driver_id = next_tick_driver_id();
      let control = ArcShared::new(TokioQuickstartControl { tick_task, executor_task });
      let driver_handle = TickDriverHandle::new(driver_id, TickDriverKind::Auto, resolution, control);
      let metadata = AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id, resolution };

      // Create runtime
      Ok(TickDriverRuntime::new(driver_handle, feed).with_auto_metadata(metadata))
    })
  }

  /// Builds a factory using the provided Tokio runtime handle.
  #[must_use]
  pub fn tokio_with_handle(handle: Handle, resolution: Duration) -> TickDriverFactoryRef<StdToolbox> {
    ArcShared::new(TokioIntervalDriverFactory::new(handle, resolution))
  }

  /// Creates a Tokio quickstart configuration with event stream metrics publishing.
  ///
  /// # Panics
  ///
  /// Panics if no Tokio runtime handle is available in the current context.
  #[must_use]
  pub fn tokio_quickstart_with_event_stream(
    resolution: Duration,
    event_stream: ArcShared<EventStreamGeneric<StdToolbox>>,
    metrics_interval: Duration,
  ) -> TickDriverConfig<StdToolbox> {
    use fraktor_utils_rs::core::runtime_toolbox::ToolboxMutex;
    use tokio::time::{MissedTickBehavior, interval};

    use crate::core::scheduler::{
      AutoDriverMetadata, AutoProfileKind, Scheduler, SchedulerTickExecutor, SchedulerTickMetricsProbe,
      TickDriverControl, TickDriverHandle, TickDriverKind, TickDriverRuntime, TickExecutorSignal, TickFeed,
      next_tick_driver_id,
    };

    TickDriverConfig::new(move |ctx| {
      let handle = Handle::try_current().expect("Tokio runtime handle unavailable");

      let scheduler: Arc<ToolboxMutex<Scheduler<StdToolbox>, StdToolbox>> = ctx.scheduler();
      let capacity = {
        let guard = scheduler.lock();
        guard.config().profile().tick_buffer_quota()
      };

      let signal = TickExecutorSignal::new();
      let feed = TickFeed::new(resolution, capacity, signal);
      let feed_clone = feed.clone();

      // Spawn tick generator task
      let tick_task = handle.spawn(async move {
        let mut ticker = interval(resolution);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
          ticker.tick().await;
          feed_clone.enqueue(1);
        }
      });

      // Spawn scheduler executor task
      let executor_feed = feed.clone();
      let executor_signal = executor_feed.signal();
      let executor_scheduler = scheduler.clone();
      let executor_task = handle.spawn(async move {
        let mut executor = SchedulerTickExecutor::new(executor_scheduler, executor_feed, executor_signal);
        loop {
          executor.drive_pending();
          tokio::time::sleep(resolution / 10).await;
        }
      });

      // Spawn metrics emitter task
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
          use crate::core::event_stream::EventStreamEvent;
          metrics_event_stream.publish(&EventStreamEvent::SchedulerTick(metrics));
        }
      });

      // Create driver handle
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
      let control = Arc::new(TokioQuickstartControl { tick_task, executor_task, metrics_task: Some(metrics_task) });
      let driver_handle = TickDriverHandle::new(driver_id, TickDriverKind::Auto, resolution, control);
      let metadata = AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id, resolution };

      Ok(TickDriverRuntime::new(driver_handle, feed).with_auto_metadata(metadata))
    })
  }

  /// Builds a factory with explicit handle and metrics publishing.
  #[must_use]
  pub fn tokio_with_handle_and_event_stream(
    handle: Handle,
    resolution: Duration,
    event_stream: ArcShared<EventStreamGeneric<StdToolbox>>,
    interval: Duration,
  ) -> TickDriverFactoryRef<StdToolbox> {
    ArcShared::new(TokioIntervalDriverFactory::new(handle, resolution).with_metrics(event_stream, interval))
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Mutex;

  use fraktor_utils_rs::core::{sync::ArcShared, time::TimerInstant};

  use crate::core::{
    event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber},
    scheduler::{AutoProfileKind, SchedulerConfig, SchedulerContext, TickDriverBootstrap, TickDriverKind},
  };
  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  #[allow(clippy::expect_used)]
  async fn tokio_interval_driver_produces_ticks() {
    let config = StdTickDriverConfig::tokio_quickstart_with_resolution(Duration::from_millis(5));
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

    let config = StdTickDriverConfig::tokio_quickstart_with_event_stream(
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
    let config = StdTickDriverConfig::tokio_quickstart();
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
}
