use super::{DriveOutcome, StreamError, StreamHandleId, StreamState};

/// Stream handle contract.
pub trait StreamHandle {
  /// Returns the handle identifier.
  fn id(&self) -> StreamHandleId;

  /// Returns the current stream state.
  fn state(&self) -> StreamState;

  /// Cancels the stream execution.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when cancellation fails.
  fn cancel(&self) -> Result<(), StreamError>;

  /// Drives the stream once.
  fn drive(&self) -> DriveOutcome;
}
