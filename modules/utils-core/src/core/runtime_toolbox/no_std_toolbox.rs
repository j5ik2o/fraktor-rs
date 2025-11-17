use core::time::Duration;

use super::RuntimeToolbox;
use crate::core::{
  runtime_toolbox::sync_mutex_family::SpinMutexFamily,
  time::{ManualClock, SchedulerTickHandle},
};

#[cfg(test)]
mod tests;

/// Default toolbox for no_std environments, backed by [`SpinMutexFamily`].
#[derive(Clone, Copy, Debug)]
pub struct NoStdToolbox {
  clock: ManualClock,
}

impl NoStdToolbox {
  /// Creates a toolbox with the provided clock resolution.
  #[must_use]
  pub const fn new(resolution: Duration) -> Self {
    Self { clock: ManualClock::new(resolution) }
  }
}

impl Default for NoStdToolbox {
  fn default() -> Self {
    Self::new(Duration::from_millis(1))
  }
}

impl RuntimeToolbox for NoStdToolbox {
  type Clock = ManualClock;
  type MutexFamily = SpinMutexFamily;

  fn clock(&self) -> &Self::Clock {
    &self.clock
  }

  fn tick_source(&self) -> SchedulerTickHandle<'_> {
    SchedulerTickHandle::scoped(self)
  }
}
