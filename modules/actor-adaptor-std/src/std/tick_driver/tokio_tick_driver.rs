//! Tokio-based tick driver using `tokio::time::interval`.

extern crate std;

use alloc::boxed::Box;
use core::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};
use std::sync::{
  Arc, Mutex,
  mpsc::{Receiver, channel},
};

use fraktor_actor_core_rs::core::kernel::actor::scheduler::tick_driver::{
  AutoDriverMetadata, AutoProfileKind, SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverKind,
  TickDriverProvision, TickDriverStopper, TickFeedHandle, next_tick_driver_id,
};
use tokio::{
  runtime::{Handle, RuntimeFlavor},
  time::{MissedTickBehavior, interval},
};

/// Tokio-based tick driver using `tokio::time::interval`.
pub struct TokioTickDriver {
  resolution: Duration,
}

impl TokioTickDriver {
  /// Creates a new Tokio tick driver with the given resolution.
  #[must_use]
  pub const fn new(resolution: Duration) -> Self {
    Self { resolution }
  }
}

impl Default for TokioTickDriver {
  fn default() -> Self {
    Self { resolution: Duration::from_millis(10) }
  }
}

impl TickDriver for TokioTickDriver {
  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Tokio
  }

  fn provision(
    self: Box<Self>,
    feed: TickFeedHandle,
    executor: SchedulerTickExecutor,
  ) -> Result<TickDriverProvision, TickDriverError> {
    let resolution = self.resolution;
    let id = next_tick_driver_id();
    let handle = Handle::try_current().map_err(|_| TickDriverError::HandleUnavailable)?;

    if handle.runtime_flavor() == RuntimeFlavor::CurrentThread {
      return Err(TickDriverError::UnsupportedRuntime);
    }

    let running = Arc::new(AtomicBool::new(true));
    let (done_tx, done_rx) = channel::<()>();

    let tick_running = running.clone();
    let tick_done = done_tx.clone();
    let _tick_task = handle.spawn(async move {
      let mut ticker = interval(resolution);
      ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
      loop {
        ticker.tick().await;
        if !tick_running.load(Ordering::Acquire) {
          break;
        }
        feed.enqueue(1);
      }
      let _ = tick_done.send(());
    });

    let exec_running = running.clone();
    let exec_done = done_tx;
    let exec_interval = resolution / 10;
    let mut executor = executor;
    let _exec_task = handle.spawn(async move {
      loop {
        if !exec_running.load(Ordering::Acquire) {
          break;
        }
        executor.drive_pending();
        tokio::time::sleep(exec_interval).await;
      }
      let _ = exec_done.send(());
    });

    Ok(TickDriverProvision {
      resolution,
      id,
      kind: TickDriverKind::Tokio,
      stopper: Box::new(TokioTickDriverStopper { running, done_rx: Mutex::new(done_rx) }),
      auto_metadata: Some(AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id: id, resolution }),
    })
  }
}

struct TokioTickDriverStopper {
  running: Arc<AtomicBool>,
  done_rx: Mutex<Receiver<()>>,
}

impl TickDriverStopper for TokioTickDriverStopper {
  fn stop(self: Box<Self>) {
    self.running.store(false, Ordering::Release);
    if let Ok(rx) = self.done_rx.into_inner() {
      let _ = rx.recv();
      let _ = rx.recv();
    }
  }
}
