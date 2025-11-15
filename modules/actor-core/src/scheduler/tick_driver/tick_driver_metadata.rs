//! Metadata for active tick driver configuration.

#[cfg(test)]
mod tests;

use core::time::Duration;

use fraktor_utils_core_rs::time::TimerInstant;

/// Unique identifier for a tick driver instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TickDriverId(u64);

impl TickDriverId {
  /// Creates a new tick driver ID.
  #[must_use]
  pub const fn new(id: u64) -> Self {
    Self(id)
  }

  /// Returns the underlying ID value.
  #[must_use]
  pub const fn as_u64(self) -> u64 {
    self.0
  }
}

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

/// Metadata for automatic driver profile selection.
#[derive(Debug, Clone)]
pub struct AutoDriverMetadata {
  /// Auto-detection profile used.
  pub profile:    AutoProfileKind,
  /// Selected driver ID.
  pub driver_id:  TickDriverId,
  /// Configured tick resolution.
  pub resolution: Duration,
}

/// Classification of auto-detected driver profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoProfileKind {
  /// Tokio runtime detected.
  Tokio,
  /// Embassy runtime detected.
  Embassy,
  /// Custom runtime.
  Custom,
}
