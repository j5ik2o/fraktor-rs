#[cfg(test)]
mod tests;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::StreamHandleGeneric;

/// Commands processed by the stream drive actor.
#[derive(Clone)]
pub(crate) enum StreamDriveCommand<TB: RuntimeToolbox + 'static> {
  /// Registers a new stream handle for periodic driving.
  Register {
    /// Stream handle to register.
    handle: StreamHandleGeneric<TB>,
  },
  /// Drives all registered streams once.
  Tick,
  /// Cancels all streams and shuts down.
  Shutdown,
}
