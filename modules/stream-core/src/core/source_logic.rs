use crate::core::{DynValue, StreamError, stream_ref::StreamRefSettings};

/// Source-stage callback contract used by adaptor implementations.
///
/// [`pull`](SourceLogic::pull) returns `Ok(Some(value))` for each produced
/// element and `Ok(None)` to signal stream completion.  The returned
/// [`DynValue`] must hold the concrete type expected by the downstream
/// `Source<Out, _>` (i.e. the same `Out` type parameter).  A type mismatch
/// will result in a `TypeMismatch` error at the adaptor boundary.
pub trait SourceLogic: Send {
  /// Produces the next output element, or `None` to signal completion.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when the source cannot provide the next element.
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError>;

  /// Handles downstream cancellation.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when cancellation cleanup fails.
  fn on_cancel(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  /// Returns whether this source should keep pulling already accepted data
  /// during graph-wide shutdown.
  #[must_use]
  fn should_drain_on_shutdown(&self) -> bool {
    false
  }

  /// Handles graph-wide shutdown.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when shutdown cleanup fails.
  fn on_shutdown(&mut self) -> Result<(), StreamError> {
    self.on_cancel()
  }

  /// Resets internal state after a restart decision.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when restart cleanup fails.
  fn on_restart(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  /// Attaches stream reference settings resolved by the materializer.
  fn attach_stream_ref_settings(&mut self, _settings: StreamRefSettings) {}
}
