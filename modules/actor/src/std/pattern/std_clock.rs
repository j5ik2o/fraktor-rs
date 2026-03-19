//! Standard-library clock backed by [`std::time::Instant`].

extern crate std;

use core::time::Duration;
use std::time::Instant;

use crate::core::pattern::Clock;

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
