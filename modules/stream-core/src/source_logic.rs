use crate::{DynValue, StreamError, stream_ref::StreamRefSettings};

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

  /// Returns whether this source should keep pulling data during graph-wide
  /// shutdown.
  ///
  /// Finite sources should drain by default so graceful shutdown does not drop
  /// already accepted data. Infinite or externally driven sources must override
  /// this to `false` when shutdown should cancel them immediately.
  #[must_use]
  fn should_drain_on_shutdown(&self) -> bool {
    true
  }

  /// Handles graph-wide shutdown before any non-draining source is cancelled.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when shutdown cleanup fails.
  fn on_shutdown(&mut self) -> Result<(), StreamError> {
    Ok(())
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
