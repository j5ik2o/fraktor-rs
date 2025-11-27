//! Hardware timer pulse source abstraction.

use core::time::Duration;

use super::{TickDriverError, TickPulseHandler};

/// Abstraction for hardware timer tick sources.
pub trait TickPulseSource: Send + Sync + 'static {
  /// Enables the hardware timer.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError`] if timer initialization fails.
  fn enable(&mut self) -> Result<(), TickDriverError>;

  /// Disables the hardware timer.
  fn disable(&mut self);

  /// Sets the callback handler for tick events.
  fn set_callback(&mut self, handler: TickPulseHandler);

  /// Returns the tick resolution of this timer.
  fn resolution(&self) -> Duration;
}
