use crate::core::{DynValue, StreamError};

/// Source-stage callback contract used by adaptor implementations.
pub trait SourceLogic: Send {
  /// Produces the next output element.
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

  /// Resets internal state after a restart decision.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when restart cleanup fails.
  fn on_restart(&mut self) -> Result<(), StreamError> {
    Ok(())
  }
}
