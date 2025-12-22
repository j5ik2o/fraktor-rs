//! Stream state definitions.

/// Execution state of a stream handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
  /// Stream has not started yet.
  Idle,
  /// Stream is running.
  Running,
  /// Stream completed successfully.
  Completed,
  /// Stream failed.
  Failed,
  /// Stream was cancelled.
  Cancelled,
}
