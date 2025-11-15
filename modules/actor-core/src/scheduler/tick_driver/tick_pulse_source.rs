//! Hardware timer pulse source abstraction.

use core::time::Duration;

use super::TickDriverError;

/// Callback handler for tick pulses from hardware timers.
#[repr(C)]
pub struct TickPulseHandler {
  /// Function pointer for the callback.
  pub func: unsafe extern "C" fn(*mut core::ffi::c_void),
  /// Context pointer passed to the callback.
  pub ctx:  *mut core::ffi::c_void,
}

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
