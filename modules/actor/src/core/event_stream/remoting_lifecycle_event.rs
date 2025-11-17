//! Remoting lifecycle notifications pushed through the event stream.

use alloc::string::String;

/// Lifecycle event emitted by the remoting subsystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemotingLifecycleEvent {
  /// Remoting is preparing to start.
  Starting,
  /// Remoting finished startup procedures.
  Started,
  /// Remoting is shutting down or already stopped.
  Shutdown,
  /// Remoting encountered a fatal error.
  Error {
    /// Describes the error that forced remoting to stop.
    message: String,
  },
}
