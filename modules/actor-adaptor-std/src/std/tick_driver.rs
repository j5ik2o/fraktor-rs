//! Tokio-based tick driver implementations for standard runtimes.

extern crate std;

#[cfg(feature = "tokio-executor")]
use alloc::boxed::Box;
#[cfg(feature = "tokio-executor")]
use core::time::Duration;

#[cfg(feature = "tokio-executor")]
use fraktor_actor_core_rs::core::kernel::actor::scheduler::tick_driver::{
  AutoDriverMetadata, AutoProfileKind, SchedulerTickExecutor, TickDriver, TickDriverConfig as CoreTickDriverConfig,
  TickDriverControl, TickDriverError, TickDriverHandle, TickDriverId, TickDriverKind, TickExecutorPump, TickFeedHandle,
  next_tick_driver_id,
};
#[cfg(feature = "tokio-executor")]
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};
#[cfg(feature = "tokio-executor")]
use tokio::{
  runtime::Handle,
  task::JoinHandle,
  time::{MissedTickBehavior, interval},
};

#[cfg(all(test, feature = "tokio-executor"))]
mod tests;

/// Creates a ready-to-use tick driver configuration with the default 10ms resolution.
#[cfg(feature = "tokio-executor")]
#[must_use]
pub fn default_tick_driver_config() -> CoreTickDriverConfig {
  tick_driver_config_with_resolution(Duration::from_millis(10))
}

/// Creates a Tokio tick driver configuration with custom resolution.
#[cfg(feature = "tokio-executor")]
#[must_use]
pub fn tick_driver_config_with_resolution(resolution: Duration) -> CoreTickDriverConfig {
  CoreTickDriverConfig::runtime(
    Box::new(TokioTickDriver::new(resolution)),
    Box::new(TokioTickExecutorPump::new(resolution)),
  )
}

#[cfg(feature = "tokio-executor")]
struct TokioTickDriver {
  id:         TickDriverId,
  resolution: Duration,
}

#[cfg(feature = "tokio-executor")]
impl TokioTickDriver {
  fn new(resolution: Duration) -> Self {
    Self { id: next_tick_driver_id(), resolution }
  }
}

#[cfg(feature = "tokio-executor")]
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
    let control = SharedLock::new_with_driver::<SpinSyncMutex<_>>(control);
    Ok(TickDriverHandle::new(self.id, TickDriverKind::Auto, resolution, control))
  }
}

#[cfg(feature = "tokio-executor")]
struct TokioTickDriverControl {
  tick_task: JoinHandle<()>,
}

#[cfg(feature = "tokio-executor")]
impl TickDriverControl for TokioTickDriverControl {
  fn shutdown(&self) {
    self.tick_task.abort();
  }
}

#[cfg(feature = "tokio-executor")]
struct TokioTickExecutorPump {
  resolution: Duration,
}

#[cfg(feature = "tokio-executor")]
impl TokioTickExecutorPump {
  const fn new(resolution: Duration) -> Self {
    Self { resolution }
  }
}

#[cfg(feature = "tokio-executor")]
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

#[cfg(feature = "tokio-executor")]
struct TokioTickExecutorControl {
  executor_task: JoinHandle<()>,
}

#[cfg(feature = "tokio-executor")]
impl TickDriverControl for TokioTickExecutorControl {
  fn shutdown(&self) {
    self.executor_task.abort();
  }
}
