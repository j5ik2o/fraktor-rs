//! DelayProvider implementation backed by the scheduler.

use core::time::Duration;

use fraktor_utils_rs::core::{
  sync::{ArcShared, SharedAccess},
  timing::delay::{DelayFuture, DelayProvider, DelayTrigger},
};

use super::{
  Scheduler, SchedulerCommand, SchedulerError, SchedulerHandle, SchedulerShared, execution_batch::ExecutionBatch,
  runnable::SchedulerRunnable,
};

/// Provides delay futures by scheduling runnable tasks on the canonical scheduler.
///
/// # Interior Mutability Removed
///
/// This implementation now requires `&mut self` for the `delay()` method.
/// The internal `Scheduler` is still protected by a mutex because it is a shared
/// system resource, but callers must ensure exclusive access to this provider.
pub struct SchedulerBackedDelayProvider {
  scheduler: SchedulerShared,
}

impl SchedulerBackedDelayProvider {
  /// Creates a provider referencing the shared scheduler instance.
  #[must_use]
  pub const fn new(scheduler: SchedulerShared) -> Self {
    Self { scheduler }
  }

  fn with_scheduler<R>(&mut self, f: impl FnOnce(&mut Scheduler) -> R) -> R {
    self.scheduler.with_write(f)
  }

  fn schedule_delay(&mut self, duration: Duration, trigger: &DelayTrigger) -> Result<SchedulerHandle, SchedulerError> {
    let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(TriggerRunnable { trigger: trigger.clone() });
    self.with_scheduler(|scheduler| {
      scheduler
        .schedule_command(duration, SchedulerCommand::RunRunnable { runnable: runnable.clone() })
    })
  }

  fn install_cancel_hook(&self, handle: SchedulerHandle, trigger: &DelayTrigger) {
    let scheduler = self.scheduler.clone();
    trigger.set_cancel_hook(move || {
      scheduler.with_write(|s| {
        let _ = s.cancel(&handle);
      });
    });
  }
}

impl Clone for SchedulerBackedDelayProvider {
  fn clone(&self) -> Self {
    Self { scheduler: self.scheduler.clone() }
  }
}

impl DelayProvider for SchedulerBackedDelayProvider {
  fn delay(&mut self, duration: Duration) -> DelayFuture {
    let (future, trigger) = DelayFuture::new_pair(duration);
    match self.schedule_delay(duration, &trigger) {
      | Ok(handle) => {
        self.install_cancel_hook(handle, &trigger);
      },
      | Err(_) => {
        trigger.fire();
      },
    }
    future
  }
}

struct TriggerRunnable {
  trigger: DelayTrigger,
}

impl SchedulerRunnable for TriggerRunnable {
  fn run(&self, _batch: &ExecutionBatch) {
    self.trigger.fire();
  }
}
