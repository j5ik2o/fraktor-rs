//! Standard-library clock backed by [`std::time::Instant`].

#[cfg(test)]
mod tests;

extern crate std;

use core::time::Duration;
use std::time::Instant;

use fraktor_actor_core_kernel_rs::pattern::Clock;

/// A [`Clock`] implementation backed by the standard library's monotonic clock.
#[derive(Debug, Clone, Copy)]
pub struct StdClock;

impl Clock for StdClock {
  type Instant = Instant;

  #[inline]
  fn now(&self) -> Self::Instant {
    Instant::now()
  }

  #[inline]
  fn elapsed_since(&self, earlier: Self::Instant) -> Duration {
    earlier.elapsed()
  }
}
