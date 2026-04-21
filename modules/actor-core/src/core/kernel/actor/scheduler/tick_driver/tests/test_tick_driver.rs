//! Test tick driver for deterministic testing.
// std::thread is required for test-support functionality; exempt from no_std restriction.
#![allow(cfg_std_forbid)]

extern crate std;

use alloc::boxed::Box;
use core::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};
use std::thread::{self, JoinHandle};

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::super::{
  SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverKind, TickDriverProvision, TickDriverStopper,
  TickFeedHandle, next_tick_driver_id,
};

/// Test tick driver that uses `std::thread` + `sleep` for driving.
///
/// Returns [`TickDriverKind::Manual`] so that `build_from_owned_config`
/// auto-enables `runner_api_enabled` before provisioning.
pub(crate) struct TestTickDriver {
  resolution: Duration,
}

impl Default for TestTickDriver {
  fn default() -> Self {
    Self { resolution: Duration::from_millis(10) }
  }
}

impl TickDriver for TestTickDriver {
  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Manual
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
    let running = ArcShared::new(AtomicBool::new(true));

    let tick_flag = running.clone();
    let tick_thread = thread::Builder::new()
      .name("test-tick-driver-tick".into())
      .spawn(move || {
        loop {
          thread::sleep(resolution);
          if !tick_flag.load(Ordering::Acquire) {
            break;
          }
          feed.enqueue(1);
        }
      })
      .map_err(|_| TickDriverError::SpawnFailed)?;

    let exec_flag = running.clone();
    let exec_interval = (resolution / 10).max(Duration::from_millis(1));
    let mut executor = executor;
    let exec_thread = match thread::Builder::new().name("test-tick-driver-exec".into()).spawn(move || {
      loop {
        if !exec_flag.load(Ordering::Acquire) {
          break;
        }
        executor.drive_pending();
        thread::sleep(exec_interval);
      }
    }) {
      | Ok(handle) => handle,
      | Err(_) => {
        running.store(false, Ordering::Release);
        if tick_thread.join().is_err() {
          std::eprintln!("warn: test tick driver tick thread panicked during spawn-failure cleanup");
        }
        return Err(TickDriverError::SpawnFailed);
      },
    };

    Ok(TickDriverProvision {
      resolution,
      id,
      kind: TickDriverKind::Manual,
      stopper: Box::new(TestTickDriverStopper {
        running,
        tick_thread: Some(tick_thread),
        exec_thread: Some(exec_thread),
      }),
      auto_metadata: None,
    })
  }
}

struct TestTickDriverStopper {
  running:     ArcShared<AtomicBool>,
  tick_thread: Option<JoinHandle<()>>,
  exec_thread: Option<JoinHandle<()>>,
}

impl TickDriverStopper for TestTickDriverStopper {
  fn stop(mut self: Box<Self>) {
    self.running.store(false, Ordering::Release);
    if let Some(h) = self.tick_thread.take()
      && h.join().is_err()
    {
      std::eprintln!("warn: test tick driver tick thread panicked during shutdown");
    }
    if let Some(h) = self.exec_thread.take()
      && h.join().is_err()
    {
      std::eprintln!("warn: test tick driver executor thread panicked during shutdown");
    }
  }
}
