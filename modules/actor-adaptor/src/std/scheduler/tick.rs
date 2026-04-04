//! Tokio-based tick driver implementations for std runtimes.

extern crate std;

use std::time::Duration;

use fraktor_actor_rs::core::kernel::actor::scheduler::tick_driver::TickDriverConfig as CoreTickDriverConfig;
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};
use tokio::runtime::Handle;

#[cfg(test)]
mod tests;

/// Config helpers for std tick drivers.
pub(crate) struct TickDriverConfig;

impl TickDriverConfig {
  /// Creates a ready-to-use tick driver configuration with the default 10ms resolution.
  ///
  /// # Panics
  ///
  /// Panics if no Tokio runtime handle is available in the current context.
  #[must_use]
  pub(crate) fn default_config() -> CoreTickDriverConfig {
    Self::with_resolution(Duration::from_millis(10))
  }

  /// Creates a Tokio tick driver configuration with custom resolution.
  ///
  /// This creates a complete tick driver setup including both the tick generator
  /// and the scheduler executor, similar to no_std hardware tick driver patterns.
  ///
  /// # Panics
  ///
  /// Panics if no Tokio runtime handle is available in the current context.
  #[must_use]
  pub(crate) fn with_resolution(resolution: Duration) -> CoreTickDriverConfig {
    use alloc::boxed::Box;

    use fraktor_actor_rs::core::kernel::actor::scheduler::{
      SchedulerShared,
      tick_driver::{
        AutoDriverMetadata, AutoProfileKind, SchedulerTickExecutor, TickDriverBundle, TickDriverControl,
        TickDriverHandle, TickDriverKind, TickExecutorSignal, TickFeed, next_tick_driver_id,
      },
    };
    use tokio::time::{MissedTickBehavior, interval};

    CoreTickDriverConfig::new(move |ctx| {
      #[allow(clippy::expect_used)]
      let handle = Handle::try_current().expect("Tokio runtime handle unavailable");

      // Get scheduler, resolution, and capacity from context
      let scheduler: SchedulerShared = ctx.scheduler();
      let capacity = scheduler.with_read(|s| s.config().profile().tick_buffer_quota());

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
      let executor_task = handle.spawn(async move {
        let mut executor = SchedulerTickExecutor::new(scheduler, executor_feed, executor_signal);
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
      let control: Box<dyn TickDriverControl> = Box::new(TokioQuickstartControl { tick_task, executor_task });
      let control = ArcShared::new(RuntimeMutex::new(control));
      let driver_handle = TickDriverHandle::new(driver_id, TickDriverKind::Auto, resolution, control);
      let metadata = AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id, resolution };

      // Create runtime
      Ok(TickDriverBundle::new(driver_handle, feed).with_auto_metadata(metadata))
    })
  }
}
