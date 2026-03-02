#[cfg(test)]
mod tests;

use super::StreamHandleImpl;

/// Commands processed by the stream drive actor.
#[derive(Clone)]
pub(crate) enum StreamDriveCommand {
  /// Registers a new stream handle for periodic driving.
  Register {
    /// Stream handle to register.
    handle: StreamHandleImpl,
  },
  /// Drives all registered streams once.
  Tick,
  /// Cancels all streams and shuts down.
  Shutdown,
}
