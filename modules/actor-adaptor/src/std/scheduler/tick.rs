//! Tokio-based tick driver implementations for std runtimes.

extern crate std;

use alloc::boxed::Box;
use core::time::Duration;

use fraktor_actor_rs::core::kernel::actor::scheduler::tick_driver::{
  AutoDriverMetadata, AutoProfileKind, SchedulerTickExecutor, TickDriver, TickDriverConfig as CoreTickDriverConfig,
  TickDriverControl, TickDriverError, TickDriverHandle, TickDriverId, TickDriverKind, TickExecutorPump, TickFeedHandle,
  next_tick_driver_id,
};
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};
use tokio::{
  runtime::Handle,
  time::{MissedTickBehavior, interval},
};

#[cfg(test)]
mod tests;

/// Config helpers for std tick drivers.
pub(crate) struct TickDriverConfig;

impl TickDriverConfig {
  /// Creates a ready-to-use tick driver configuration with the default 10ms resolution.
  #[must_use]
  pub(crate) fn default_config() -> CoreTickDriverConfig {
    Self::with_resolution(Duration::from_millis(10))
  }

  /// Creates a Tokio tick driver configuration with custom resolution.
  #[must_use]
  pub(crate) fn with_resolution(resolution: Duration) -> CoreTickDriverConfig {
    CoreTickDriverConfig::runtime(
      Box::new(TokioTickDriver::new(resolution)),
      Box::new(TokioTickExecutorPump::new(resolution)),
    )
  }
}

struct TokioTickDriver {
  id:         TickDriverId,
  resolution: Duration,
}

impl TokioTickDriver {
  fn new(resolution: Duration) -> Self {
    Self { id: next_tick_driver_id(), resolution }
  }
}

impl TickDriver for TokioTickDriver {
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
    let resolution = self.resolution;
    let tick_task = handle.spawn(async move {
      let mut ticker = interval(resolution);
      ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
      loop {
        ticker.tick().await;
        feed.enqueue(1);
      }
    });

    let control: Box<dyn TickDriverControl> = Box::new(TokioTickDriverControl { tick_task });
    let control = ArcShared::new(RuntimeMutex::new(control));
    Ok(TickDriverHandle::new(self.id, TickDriverKind::Auto, resolution, control))
  }
}

struct TokioTickDriverControl {
  tick_task: tokio::task::JoinHandle<()>,
}

impl TickDriverControl for TokioTickDriverControl {
  fn shutdown(&self) {
    self.tick_task.abort();
  }
}

struct TokioTickExecutorPump {
  resolution: Duration,
}

impl TokioTickExecutorPump {
  const fn new(resolution: Duration) -> Self {
    Self { resolution }
  }
}

impl TickExecutorPump for TokioTickExecutorPump {
  fn spawn(&mut self, mut executor: SchedulerTickExecutor) -> Result<Box<dyn TickDriverControl>, TickDriverError> {
    let handle = Handle::try_current().map_err(|_| TickDriverError::HandleUnavailable)?;
    let resolution = self.resolution;
    let executor_task = handle.spawn(async move {
      loop {
        executor.drive_pending();
        tokio::time::sleep(resolution / 10).await;
      }
    });
    Ok(Box::new(TokioTickExecutorControl { executor_task }))
  }

  fn auto_metadata(&self, driver_id: TickDriverId, resolution: Duration) -> Option<AutoDriverMetadata> {
    Some(AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id, resolution })
  }
}

struct TokioTickExecutorControl {
  executor_task: tokio::task::JoinHandle<()>,
}

impl TickDriverControl for TokioTickExecutorControl {
  fn shutdown(&self) {
    self.executor_task.abort();
  }
}
