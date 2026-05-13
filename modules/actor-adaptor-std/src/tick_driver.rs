//! Tick driver implementations for standard (std) environments.

extern crate std;

#[cfg(feature = "test-support")]
mod test_tick_driver;
#[cfg(feature = "tokio-executor")]
mod tokio_tick_driver;

#[cfg(test)]
#[path = "tick_driver_test.rs"]
mod tests;

use alloc::boxed::Box;
use core::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};
use std::{sync::Arc, thread, thread::JoinHandle};

use fraktor_actor_core_kernel_rs::actor::scheduler::tick_driver::{
  SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverKind, TickDriverProvision, TickDriverStopper,
  TickFeedHandle, next_tick_driver_id,
};
#[cfg(feature = "test-support")]
pub use test_tick_driver::TestTickDriver;
#[cfg(feature = "tokio-executor")]
pub use tokio_tick_driver::TokioTickDriver;

/// Tick driver backed by `std::thread` + `thread::sleep`.
pub struct StdTickDriver {
  resolution: Duration,
}

impl StdTickDriver {
  /// Creates a new std tick driver with the given resolution.
  #[must_use]
  pub const fn new(resolution: Duration) -> Self {
    Self { resolution }
  }
}

impl Default for StdTickDriver {
  fn default() -> Self {
    Self { resolution: Duration::from_millis(10) }
  }
}

impl TickDriver for StdTickDriver {
  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Std
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
    let id = next_tick_driver_id();
    let running = Arc::new(AtomicBool::new(true));

    let tick_flag = running.clone();
    let tick_thread = thread::spawn(move || {
      loop {
        thread::sleep(resolution);
        if !tick_flag.load(Ordering::Acquire) {
          break;
        }
        feed.enqueue(1);
      }
    });

    let exec_flag = running.clone();
    let exec_interval = (resolution / 10).max(Duration::from_millis(1));
    let mut executor = executor;
    let exec_thread = thread::spawn(move || {
      loop {
        if !exec_flag.load(Ordering::Acquire) {
          break;
        }
        executor.drive_pending();
        thread::sleep(exec_interval);
      }
    });

    Ok(TickDriverProvision {
      resolution,
      id,
      kind: TickDriverKind::Std,
      stopper: Box::new(StdTickDriverStopper {
        running,
        tick_thread: Some(tick_thread),
        exec_thread: Some(exec_thread),
      }),
      auto_metadata: None,
    })
  }
}

struct StdTickDriverStopper {
  running:     Arc<AtomicBool>,
  tick_thread: Option<JoinHandle<()>>,
  exec_thread: Option<JoinHandle<()>>,
}

impl TickDriverStopper for StdTickDriverStopper {
  fn stop(mut self: Box<Self>) {
    self.running.store(false, Ordering::Release);
    if let Some(h) = self.tick_thread.take()
      && h.join().is_err()
    {
      std::eprintln!("warn: tick driver tick thread panicked during shutdown");
    }
    if let Some(h) = self.exec_thread.take()
      && h.join().is_err()
    {
      std::eprintln!("warn: tick driver executor thread panicked during shutdown");
    }
  }
}
