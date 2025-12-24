//! Snapshot describing the active tick driver.

use core::time::Duration;

use crate::core::scheduler::{AutoDriverMetadata, TickDriverKind, TickDriverMetadata};

/// Snapshot describing the active tick driver.
#[derive(Clone, Debug)]
pub struct TickDriverSnapshot {
  /// Core driver metadata.
  pub metadata:   TickDriverMetadata,
  /// Driver classification.
  pub kind:       TickDriverKind,
  /// Configured resolution.
  pub resolution: Duration,
  /// Auto driver metadata when available.
  pub auto:       Option<AutoDriverMetadata>,
}

impl TickDriverSnapshot {
  /// Creates a new snapshot instance.
  #[must_use]
  pub const fn new(
    metadata: TickDriverMetadata,
    kind: TickDriverKind,
    resolution: Duration,
    auto: Option<AutoDriverMetadata>,
  ) -> Self {
    Self { metadata, kind, resolution, auto }
  }
}
