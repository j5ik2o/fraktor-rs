//! Metadata describing auto-selected drivers.

use core::time::Duration;

use super::{AutoProfileKind, TickDriverId};

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
