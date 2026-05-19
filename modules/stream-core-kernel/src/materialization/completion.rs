use super::StreamError;

/// Polling result for stream completions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completion<T> {
  /// Completion is still pending.
  Pending,
  /// Completion is ready with the provided result.
  Ready(Result<T, StreamError>),
}
