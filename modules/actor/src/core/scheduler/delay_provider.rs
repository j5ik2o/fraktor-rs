//! DelayProvider implementation backed by the scheduler.

use core::time::Duration;

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
  timing::{DelayFuture, DelayProvider, DelayTrigger},
};

use super::{
  Scheduler, SchedulerCommand, SchedulerError, SchedulerHandle, execution_batch::ExecutionBatch,
  runnable::SchedulerRunnable,
};

/// Provides delay futures by scheduling runnable tasks on the canonical scheduler.
///
/// # Interior Mutability Removed
///
/// This implementation now requires `&mut self` for the `delay()` method.
/// The internal `Scheduler` is still protected by a mutex because it is a shared
/// system resource, but callers must ensure exclusive access to this provider.
pub struct SchedulerBackedDelayProvider<TB: RuntimeToolbox + 'static> {
  scheduler: ArcShared<ToolboxMutex<Scheduler<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> SchedulerBackedDelayProvider<TB> {
  /// Creates a provider referencing the shared scheduler instance.
  #[must_use]
  pub const fn new(scheduler: ArcShared<ToolboxMutex<Scheduler<TB>, TB>>) -> Self {
    Self { scheduler }
  }

  fn with_scheduler<R>(&mut self, f: impl FnOnce(&mut Scheduler<TB>) -> R) -> R {
    let mut guard = self.scheduler.lock();
    f(&mut guard)
  }

  fn schedule_delay(&mut self, duration: Duration, trigger: &DelayTrigger) -> Result<SchedulerHandle, SchedulerError> {
    let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(TriggerRunnable { trigger: trigger.clone() });
    self.with_scheduler(|scheduler| {
      scheduler
        .schedule_command(duration, SchedulerCommand::RunRunnable { runnable: runnable.clone(), dispatcher: None })
    })
  }

  fn install_cancel_hook(&self, handle: SchedulerHandle, trigger: &DelayTrigger) {
    let scheduler = self.scheduler.clone();
    trigger.set_cancel_hook(move || {
      let mut guard = scheduler.lock();
      let _ = guard.cancel(&handle);
    });
  }
}

impl<TB: RuntimeToolbox + 'static> Clone for SchedulerBackedDelayProvider<TB> {
  fn clone(&self) -> Self {
    Self { scheduler: self.scheduler.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> DelayProvider for SchedulerBackedDelayProvider<TB> {
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
