/// Lifecycle state of a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
  /// Stream is idle before start.
  Idle,
  /// Stream is running.
  Running,
  /// Stream completed successfully.
  Completed,
  /// Stream failed with an error.
  Failed,
  /// Stream was cancelled.
  Cancelled,
}

impl StreamState {
  /// Returns `true` if the stream reached a terminal state.
  #[must_use]
  pub const fn is_terminal(&self) -> bool {
    matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
  }
}
