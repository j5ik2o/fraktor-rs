#![allow(cfg_std_forbid)]

use core::time::Duration;

use crate::{
  core::{
    runtime_toolbox::RuntimeToolbox,
    time::{ManualClock, SchedulerTickHandle},
  },
  std::runtime_toolbox::StdMutexFamily,
};

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
  pub const fn new(resolution: Duration) -> Self {
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
