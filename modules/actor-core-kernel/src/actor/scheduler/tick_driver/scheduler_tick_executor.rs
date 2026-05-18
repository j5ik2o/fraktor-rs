//! Drives the core scheduler whenever ticks are available.

use fraktor_utils_core_rs::sync::SharedAccess;

use super::{
  super::{SchedulerRunnerOwned, SchedulerShared},
  TickExecutorSignal, TickFeedHandle,
};

#[cfg(test)]
#[path = "scheduler_tick_executor_test.rs"]
mod tests;

/// Executes scheduler work by draining ticks from the feed.
pub struct SchedulerTickExecutor {
  scheduler: SchedulerShared,
  feed:      TickFeedHandle,
  signal:    TickExecutorSignal,
  runner:    SchedulerRunnerOwned,
}

impl SchedulerTickExecutor {
  /// Creates a new executor bound to the provided scheduler context.
  #[must_use]
  pub fn new(scheduler: SchedulerShared, feed: TickFeedHandle, signal: TickExecutorSignal) -> Self {
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

    self.scheduler.with_write(|s| {
      self.runner.drive(s);
    });
  }

  /// Returns the associated signal for async waiting.
  #[must_use]
  pub fn signal(&self) -> TickExecutorSignal {
    self.signal.clone()
  }
}
