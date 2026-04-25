#[cfg(test)]
mod tests;

use super::CancellationCause;
use crate::core::StreamError;

/// Handles events from a dynamic sub-source outlet.
pub trait SubSourceOutletHandler<T>: Send {
  /// Called when downstream demand reaches the sub-source outlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the parent stage logic cannot satisfy the
  /// pull request.
  fn on_pull(&mut self) -> Result<(), StreamError>;

  /// Called when downstream has cancelled the substream.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when cancellation cannot be propagated by the
  /// parent stage logic.
  fn on_downstream_finish(&mut self, _cause: CancellationCause) -> Result<(), StreamError> {
    Ok(())
  }
}
