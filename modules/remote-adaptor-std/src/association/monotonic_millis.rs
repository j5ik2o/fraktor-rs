//! Shared monotonic time conversion helpers for association runtime tasks.

use core::time::Duration;
use std::time::Instant as StdInstant;

/// Converts elapsed monotonic time since `started_at` into saturated millis.
#[must_use]
pub fn std_instant_elapsed_millis(started_at: StdInstant) -> u64 {
  duration_millis_saturated(started_at.elapsed())
}

/// Converts a duration into saturated millis.
#[must_use]
pub fn duration_millis_saturated(duration: Duration) -> u64 {
  duration.as_millis().min(u128::from(u64::MAX)) as u64
}
