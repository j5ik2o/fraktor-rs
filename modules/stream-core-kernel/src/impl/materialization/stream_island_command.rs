#[cfg(test)]
mod tests;

use crate::StreamError;

/// Commands processed by a single stream island actor.
#[derive(Clone)]
pub(crate) enum StreamIslandCommand {
  /// Drives the owned stream once.
  Drive,
  /// Cancels the owned stream.
  Cancel {
    /// Optional cancellation cause.
    cause: Option<StreamError>,
  },
  /// Cancels the owned stream during shutdown.
  Shutdown,
  /// Aborts the owned stream with an error.
  Abort(StreamError),
}
