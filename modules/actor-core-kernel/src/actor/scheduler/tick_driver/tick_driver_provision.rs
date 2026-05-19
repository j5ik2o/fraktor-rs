//! Result returned from tick driver provisioning.

use alloc::boxed::Box;
use core::time::Duration;

use super::{AutoDriverMetadata, TickDriverId, TickDriverKind, TickDriverStopper};

/// Result of a successful tick driver provisioning.
pub struct TickDriverProvision {
  /// Tick resolution.
  pub resolution:    Duration,
  /// Unique identifier for this driver instance.
  pub id:            TickDriverId,
  /// Driver classification (must match `TickDriver::kind()`).
  pub kind:          TickDriverKind,
  /// Control object for stopping the driver.
  pub stopper:       Box<dyn TickDriverStopper>,
  /// Optional auto-driver metadata (e.g. Tokio runtime info).
  pub auto_metadata: Option<AutoDriverMetadata>,
}
