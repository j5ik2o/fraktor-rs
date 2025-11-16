//! Drives the core scheduler whenever ticks are available.

use fraktor_utils_core_rs::sync::{ArcShared, sync_mutex_like::SyncMutexLike};

use super::{
  super::{Scheduler, SchedulerRunnerOwned},
  TickExecutorSignal, TickFeedHandle,
};
use crate::{RuntimeToolbox, ToolboxMutex};

#[cfg(test)]
mod tests;

/// Executes scheduler work by draining ticks from the feed.
pub struct SchedulerTickExecutor<TB: RuntimeToolbox + 'static> {
  scheduler: ArcShared<ToolboxMutex<Scheduler<TB>, TB>>,
  feed:      TickFeedHandle<TB>,
  signal:    TickExecutorSignal,
  runner:    SchedulerRunnerOwned<TB>,
}

impl<TB: RuntimeToolbox + 'static> SchedulerTickExecutor<TB> {
  /// Creates a new executor bound to the provided scheduler context.
  #[must_use]
  pub fn new(
    scheduler: ArcShared<ToolboxMutex<Scheduler<TB>, TB>>,
    feed: TickFeedHandle<TB>,
    signal: TickExecutorSignal,
  ) -> Self {
    let runner = SchedulerRunnerOwned::new();
    Self { scheduler, feed, signal, runner }
  }

  /// Drains pending ticks and advances the scheduler.
  pub fn drive_pending(&mut self) {
    let mut drained = false;
    self.feed.drain_pending(|ticks| {
      if ticks == 0 {
        return;
      }
      self.runner.inject(ticks);
      drained = true;
    });

    if !drained {
      return;
    }

    let mut guard = self.scheduler.lock();
    self.runner.drive(&mut guard);
  }

  /// Returns the associated signal for async waiting.
  #[must_use]
  pub fn signal(&self) -> TickExecutorSignal {
    self.signal.clone()
  }
}
