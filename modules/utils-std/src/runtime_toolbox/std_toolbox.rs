use core::time::Duration;

use fraktor_utils_core_rs::{
  sync::RuntimeToolbox,
  time::{ManualClock, SchedulerTickHandle},
};

use crate::runtime_toolbox::StdMutexFamily;

#[cfg(test)]
mod tests;

/// Toolbox for std environments, backed by [`StdMutexFamily`].
#[derive(Clone, Copy, Debug)]
pub struct StdToolbox {
  clock: ManualClock,
}

impl StdToolbox {
  /// Creates a toolbox whose resolution matches the desired frequency.
  #[must_use]
  pub fn new(resolution: Duration) -> Self {
    Self { clock: ManualClock::new(resolution) }
  }
}

impl Default for StdToolbox {
  fn default() -> Self {
    Self::new(Duration::from_millis(1))
  }
}

impl RuntimeToolbox for StdToolbox {
  type Clock = ManualClock;
  type MutexFamily = StdMutexFamily;

  fn clock(&self) -> &Self::Clock {
    &self.clock
  }

  fn tick_source(&self) -> SchedulerTickHandle<'_> {
    SchedulerTickHandle::scoped(self)
  }
}
