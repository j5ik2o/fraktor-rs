//! Errors raised by the remote extension surface.

use core::fmt::{Display, Formatter, Result as FmtResult};

/// Failure categories for [`crate::core::extension::Remoting`] operations and
/// [`crate::core::extension::RemotingLifecycleState`] transitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemotingError {
  /// A state transition was requested that is not permitted from the
  /// current lifecycle state.
  InvalidTransition,
  /// The transport layer is unavailable (e.g. because `start` was never
  /// called successfully, or the underlying transport refused to bind).
  TransportUnavailable,
  /// The event is recognized but concrete core handling has not been wired yet.
  UnimplementedEvent,
  /// `start` was invoked while remoting was already running.
  AlreadyRunning,
  /// A query or command requires a `Running` state but the remoting
  /// subsystem is not currently running.
  NotStarted,
}

impl Display for RemotingError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | RemotingError::InvalidTransition => f.write_str("remoting: invalid lifecycle transition"),
      | RemotingError::TransportUnavailable => f.write_str("remoting: transport unavailable"),
      | RemotingError::UnimplementedEvent => f.write_str("remoting: event handling is not implemented"),
      | RemotingError::AlreadyRunning => f.write_str("remoting: already running"),
      | RemotingError::NotStarted => f.write_str("remoting: not started"),
    }
  }
}

impl core::error::Error for RemotingError {}
