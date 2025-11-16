//! Callback hook passed to hardware pulse sources.

/// Callback registry used by [`TickPulseSource`].
#[derive(Clone, Copy)]
pub struct TickPulseHandler {
  /// Function pointer for the callback.
  pub func: unsafe extern "C" fn(*mut core::ffi::c_void),
  /// Context pointer passed to the callback.
  pub ctx:  *mut core::ffi::c_void,
}
