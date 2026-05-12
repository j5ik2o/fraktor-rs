//! Embassy tick driver using `embassy-time`.

#[cfg(test)]
#[path = "tick_driver_test.rs"]
mod tests;

use alloc::{boxed::Box, sync::Arc};
use core::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};

use embassy_executor::SendSpawner;
use embassy_time::{Duration as EmbassyDuration, Timer};
use fraktor_actor_core_kernel_rs::actor::scheduler::tick_driver::{
  AutoDriverMetadata, AutoProfileKind, SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverKind,
  TickDriverProvision, TickDriverStopper, TickFeedHandle, next_tick_driver_id,
};

/// Tick driver that can be provisioned from an Embassy executor spawner.
pub struct EmbassyTickDriver {
  resolution: Duration,
  spawner:    Option<SendSpawner>,
}

impl EmbassyTickDriver {
  /// Creates a driver with a resolution and Embassy spawner.
  #[must_use]
  pub const fn new(resolution: Duration, spawner: SendSpawner) -> Self {
    Self { resolution, spawner: Some(spawner) }
  }

  /// Creates a driver value without a spawner.
  #[must_use]
  pub const fn without_spawner(resolution: Duration) -> Self {
    Self { resolution, spawner: None }
  }
}

impl Default for EmbassyTickDriver {
  fn default() -> Self {
    Self::without_spawner(Duration::from_millis(10))
  }
}

impl TickDriver for EmbassyTickDriver {
  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Embassy
  }

  fn provision(
    self: Box<Self>,
    feed: TickFeedHandle,
    executor: SchedulerTickExecutor,
  ) -> Result<TickDriverProvision, TickDriverError> {
    let resolution = self.resolution;
    if resolution.is_zero() {
      return Err(TickDriverError::InvalidResolution);
    }
    let Some(spawner) = self.spawner else {
      return Err(TickDriverError::HandleUnavailable);
    };
    let id = next_tick_driver_id();
    let running = Arc::new(AtomicBool::new(true));
    let embassy_resolution = duration_to_embassy(resolution);
    let exec_interval = duration_to_embassy((resolution / 10).max(Duration::from_millis(1)));

    let tick_task =
      embassy_tick_task(running.clone(), feed, embassy_resolution).map_err(|_| TickDriverError::SpawnFailed)?;
    let executor_task =
      embassy_tick_executor_task(running.clone(), executor, exec_interval).map_err(|_| TickDriverError::SpawnFailed)?;
    spawner.spawn(tick_task);
    spawner.spawn(executor_task);

    Ok(TickDriverProvision {
      resolution,
      id,
      kind: TickDriverKind::Embassy,
      stopper: Box::new(EmbassyTickDriverStopper { running }),
      auto_metadata: Some(AutoDriverMetadata { profile: AutoProfileKind::Embassy, driver_id: id, resolution }),
    })
  }
}

struct EmbassyTickDriverStopper {
  running: Arc<AtomicBool>,
}

impl TickDriverStopper for EmbassyTickDriverStopper {
  fn stop(self: Box<Self>) {
    self.running.store(false, Ordering::Release);
  }
}

fn duration_to_embassy(duration: Duration) -> EmbassyDuration {
  EmbassyDuration::from_micros(duration.as_micros().min(u128::from(u64::MAX)) as u64)
}

#[embassy_executor::task]
async fn embassy_tick_task(running: Arc<AtomicBool>, feed: TickFeedHandle, resolution: EmbassyDuration) {
  loop {
    Timer::after(resolution).await;
    if !running.load(Ordering::Acquire) {
      break;
    }
    feed.enqueue(1);
  }
}

#[embassy_executor::task]
async fn embassy_tick_executor_task(
  running: Arc<AtomicBool>,
  mut executor: SchedulerTickExecutor,
  interval: EmbassyDuration,
) {
  loop {
    if !running.load(Ordering::Acquire) {
      break;
    }
    executor.drive_pending();
    Timer::after(interval).await;
  }
}
