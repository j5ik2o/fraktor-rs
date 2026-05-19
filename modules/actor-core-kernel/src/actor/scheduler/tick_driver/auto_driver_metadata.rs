//! Metadata describing auto-selected drivers.

// Issue #413: AutoProfileKind は AutoDriverMetadata のフィールド型としてのみ使用されるため同居させる。
#![allow(multiple_type_definitions)]

use core::time::Duration;

use super::TickDriverId;

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
