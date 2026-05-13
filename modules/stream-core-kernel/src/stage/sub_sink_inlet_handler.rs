#[cfg(test)]
#[path = "sub_sink_inlet_handler_test.rs"]
mod tests;

use crate::StreamError;

/// Handles events from a dynamic sub-sink inlet.
pub trait SubSinkInletHandler<T>: Send {
  /// Called when an element has arrived and can be read through the inlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the pushed element cannot be accepted by the
  /// parent stage logic.
  fn on_push(&mut self) -> Result<(), StreamError>;

  /// Called when the upstream side of the substream has completed.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when completion cannot be propagated by the parent
  /// stage logic.
  fn on_upstream_finish(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  /// Called when the upstream side of the substream has failed.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the failure is re-raised or transformed by the
  /// parent stage logic.
  fn on_upstream_failure(&mut self, error: StreamError) -> Result<(), StreamError> {
    Err(error)
  }
}
