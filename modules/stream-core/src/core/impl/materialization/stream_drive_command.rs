#[cfg(test)]
mod tests;

use super::StreamShared;

/// Commands processed by the stream drive actor.
#[derive(Clone)]
pub(crate) enum StreamDriveCommand {
  /// Registers a new stream for periodic driving.
  Register {
    /// Stream to register.
    stream: StreamShared,
  },
  /// Drives all registered streams once.
  Tick,
  /// Cancels all streams and shuts down.
  Shutdown,
}
