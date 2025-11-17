//! Metadata for active tick driver configuration.

#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::time::TimerInstant;

use super::TickDriverId;

/// Runtime metadata for the active tick driver.
#[derive(Debug, Clone)]
pub struct TickDriverMetadata {
  /// Unique identifier for this driver instance.
  pub driver_id:     TickDriverId,
  /// Timestamp when the driver was started.
  pub start_instant: TimerInstant,
  /// Total number of ticks processed.
  pub ticks_total:   u64,
}

impl TickDriverMetadata {
  /// Creates new tick driver metadata.
  #[must_use]
  pub const fn new(driver_id: TickDriverId, start_instant: TimerInstant) -> Self {
    Self { driver_id, start_instant, ticks_total: 0 }
  }
}
