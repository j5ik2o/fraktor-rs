//! Stream error definitions.

#[cfg(test)]
mod tests;

/// Errors produced by stream operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StreamError {
  /// The materializer has not been started.
  #[error("materializer not started")]
  NotStarted,
  /// The materializer has already been started.
  #[error("materializer already started")]
  AlreadyStarted,
  /// The materializer has already been shut down.
  #[error("materializer already shut down")]
  AlreadyShutdown,
  /// Connection between stages is invalid.
  #[error("invalid stream connection")]
  InvalidConnection,
  /// Demand request is invalid.
  #[error("invalid demand request")]
  InvalidDemand,
  /// The stream is not running.
  #[error("stream is not running")]
  NotRunning,
  /// Buffer reached capacity.
  #[error("buffer is full")]
  BufferFull,
  /// Buffer is closed.
  #[error("buffer is closed")]
  BufferClosed,
  /// Buffer is empty.
  #[error("buffer is empty")]
  BufferEmpty,
  /// Buffer is disconnected.
  #[error("buffer is disconnected")]
  BufferDisconnected,
  /// Buffer allocation failed.
  #[error("buffer allocation failed")]
  BufferAllocation,
  /// Buffer operation would block.
  #[error("buffer operation would block")]
  BufferWouldBlock,
  /// Required executor is unavailable.
  #[error("executor is unavailable")]
  ExecutorUnavailable,
}
