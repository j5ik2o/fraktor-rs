//! Hardware timer pulse source abstraction.

use core::time::Duration;

use super::{TickDriverError, TickPulseHandler};

/// Abstraction for hardware timer tick sources.
pub trait TickPulseSource: Send + Sync {
  /// Enables the hardware timer.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError`] if timer initialization fails.
  fn enable(&self) -> Result<(), TickDriverError>;

  /// Disables the hardware timer.
  fn disable(&self);

  /// Sets the callback handler for tick events.
  fn set_callback(&self, handler: TickPulseHandler);

  /// Returns the tick resolution of this timer.
  fn resolution(&self) -> Duration;
}
