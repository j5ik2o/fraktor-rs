//! Scheduler-backed delayed future helper.

use core::time::Duration;

use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedAccess, SharedLock};

use crate::{
  actor::scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable},
  support::futures::{ActorFuture, ActorFutureShared},
  system::state::SystemStateShared,
};

/// Creates a future that completes with `value` after `delay`.
///
/// This mirrors Pekko's `FutureTimeoutSupport.after` helper using the actor
/// scheduler as the delay source.
#[must_use]
pub fn after<T>(system: &SystemStateShared, delay: Duration, value: T) -> ActorFutureShared<T>
where
  T: Send + Sync + 'static, {
  let future = ActorFutureShared::new(ActorFuture::new());
  if delay.is_zero() {
    complete_future(&future, value);
    return future;
  }

  let value = SharedLock::new_with_driver::<DefaultMutex<_>>(Some(value));
  let task = ArcShared::new(AfterRunnable { future: future.clone(), value });
  let runnable: ArcShared<dyn SchedulerRunnable> = task.clone();
  let scheduled = system
    .scheduler()
    .with_write(|scheduler| scheduler.schedule_command(delay, SchedulerCommand::RunRunnable { runnable }));
  if scheduled.is_err() {
    task.complete();
  }
  future
}

fn complete_future<T>(future: &ActorFutureShared<T>, value: T)
where
  T: Send + 'static, {
  let waker = future.with_write(|inner| inner.complete(value));
  if let Some(waker) = waker {
    waker.wake();
  }
}

struct AfterRunnable<T>
where
  T: Send + Sync + 'static, {
  future: ActorFutureShared<T>,
  value:  SharedLock<Option<T>>,
}

impl<T> AfterRunnable<T>
where
  T: Send + Sync + 'static,
{
  fn complete(&self) {
    let value = self.value.with_write(Option::take);
    if let Some(value) = value {
      complete_future(&self.future, value);
    }
  }
}

impl<T> SchedulerRunnable for AfterRunnable<T>
where
  T: Send + Sync + 'static,
{
  fn run(&self, _batch: &ExecutionBatch) {
    self.complete();
  }
}
