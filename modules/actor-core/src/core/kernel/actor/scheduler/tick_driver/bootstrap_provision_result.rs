//! Result type for tick driver bootstrap provisioning.

use alloc::boxed::Box;

use super::{TickDriverBundle, TickDriverStopper};
use crate::core::kernel::event::stream::TickDriverSnapshot;

/// Result of a successful tick driver provisioning.
pub struct BootstrapProvisionResult {
  /// The tick driver bundle for the running driver.
  pub bundle:   TickDriverBundle,
  /// The stopper for graceful shutdown.
  pub stopper:  Box<dyn TickDriverStopper>,
  /// A snapshot of the driver state at provisioning time.
  pub snapshot: TickDriverSnapshot,
}
