//! Tokio-based tick driver using `tokio::time::interval`.

extern crate std;

use alloc::boxed::Box;
use core::{
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};
use std::sync::Arc;

use fraktor_actor_core_rs::core::kernel::actor::scheduler::tick_driver::{
  AutoDriverMetadata, AutoProfileKind, SchedulerTickExecutor, TickDriver, TickDriverError, TickDriverKind,
  TickDriverProvision, TickDriverStopper, TickFeedHandle, next_tick_driver_id,
};
use tokio::{
  runtime::{Handle, RuntimeFlavor},
  task::{JoinError, JoinHandle},
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
    if resolution.is_zero() {
      return Err(TickDriverError::InvalidResolution);
    }
    let id = next_tick_driver_id();
    let handle = Handle::try_current().map_err(|_| TickDriverError::HandleUnavailable)?;

    if handle.runtime_flavor() == RuntimeFlavor::CurrentThread {
      return Err(TickDriverError::UnsupportedExecutor);
    }

    let running = Arc::new(AtomicBool::new(true));

    let tick_running = running.clone();
    let tick_task = handle.spawn(async move {
      let mut ticker = interval(resolution);
      ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
      loop {
        ticker.tick().await;
        if !tick_running.load(Ordering::Acquire) {
          break;
        }
        feed.enqueue(1);
      }
    });

    let exec_running = running.clone();
    let exec_interval = (resolution / 10).max(Duration::from_millis(1));
    let mut executor = executor;
    let exec_task = handle.spawn(async move {
      loop {
        if !exec_running.load(Ordering::Acquire) {
          break;
        }
        executor.drive_pending();
        tokio::time::sleep(exec_interval).await;
      }
    });

    Ok(TickDriverProvision {
      resolution,
      id,
      kind: TickDriverKind::Tokio,
      stopper: Box::new(TokioTickDriverStopper { running, handle: handle.clone(), tick_task, exec_task }),
      auto_metadata: Some(AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id: id, resolution }),
    })
  }
}

struct TokioTickDriverStopper {
  running:   Arc<AtomicBool>,
  handle:    Handle,
  tick_task: JoinHandle<()>,
  exec_task: JoinHandle<()>,
}

impl TickDriverStopper for TokioTickDriverStopper {
  fn stop(self: Box<Self>) {
    self.running.store(false, Ordering::Release);
    self.tick_task.abort();
    self.exec_task.abort();
    // abort() だけでは停止要求を出すだけで、task が完全に終了したことまでは確認できない。
    // 両 task の終了を待って stop() 復帰時の停止完了を保証するため、JoinHandle を最後まで待機する。
    // Tokio runtime 内で直接 block_on すると panic するため、専用スレッドで待機する。
    let handle = self.handle;
    let tick_task = self.tick_task;
    let exec_task = self.exec_task;
    let join = std::thread::spawn(move || {
      log_task_join_result("scheduler tick task", handle.block_on(tick_task));
      log_task_join_result("scheduler executor task", handle.block_on(exec_task));
    });
    if join.join().is_err() {
      tracing::warn!("tokio tick driver stopper wait thread panicked");
    }
  }
}

fn log_task_join_result(task: &'static str, result: Result<(), JoinError>) {
  match result {
    | Ok(()) => {},
    | Err(error) if error.is_cancelled() => {},
    | Err(error) => {
      tracing::warn!(task, ?error, "tokio tick driver task finished with error");
    },
  }
}
