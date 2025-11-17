//! Error type surfaced by remoting control operations.

use alloc::string::String;
use core::fmt;

/// Errors emitted by remoting control operations.
#[derive(Debug, PartialEq, Eq)]
pub enum RemotingError {
  /// Actor system resources required by remoting are unavailable.
  SystemUnavailable,
  /// The requested operation has not been implemented yet.
  Unsupported(&'static str),
  /// Start was invoked while remoting is already running.
  AlreadyStarted,
  /// Shutdown occurred before start completed.
  NotStarted,
  /// Requested transport scheme is not available.
  TransportUnavailable(String),
  /// Operation failed due to a runtime-specific reason.
  Message(String),
}

impl RemotingError {
  /// Creates an error with a formatted message.
  #[must_use]
  pub fn message(msg: impl Into<String>) -> Self {
    Self::Message(msg.into())
  }
}

impl fmt::Display for RemotingError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::SystemUnavailable => write!(f, "remoting resources unavailable"),
      | Self::Unsupported(op) => write!(f, "operation '{op}' is not supported yet"),
      | Self::AlreadyStarted => write!(f, "remoting already started"),
      | Self::NotStarted => write!(f, "remoting not started"),
      | Self::TransportUnavailable(scheme) => {
        write!(f, "transport '{scheme}' is not available for this build")
      },
      | Self::Message(msg) => f.write_str(msg),
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for RemotingError {}
