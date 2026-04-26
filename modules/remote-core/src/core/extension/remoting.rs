//! `Remoting` port trait: pure lifecycle surface of the remote subsystem.

use crate::core::{address::Address, association::QuarantineReason, extension::remoting_error::RemotingError};

/// Lifecycle port for the remote subsystem.
///
/// This trait carries **only** the pure lifecycle responsibilities pulled
/// out of the legacy god-object `RemotingControlHandle` per design Decision
/// 5. Concrete implementations (e.g. `StdRemoting` in
/// `fraktor-remote-adaptor-std-rs`) hold the transport, bridge factory,
/// watcher daemon, and heartbeat channels — those **are not** exposed here.
///
/// All methods are synchronous. No `async fn` and no `Future` return
/// types, matching `RemoteTransport` and the `&mut self` principle of the
/// entire core crate.
pub trait Remoting {
  /// Starts the remote subsystem.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::AlreadyRunning`] if remoting is already
  /// running, or [`RemotingError::TransportUnavailable`] /
  /// [`RemotingError::InvalidTransition`] if the underlying transport
  /// could not be brought up.
  fn start(&mut self) -> Result<(), RemotingError>;

  /// Shuts the remote subsystem down.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] if remoting was never running.
  fn shutdown(&mut self) -> Result<(), RemotingError>;

  /// Quarantines the given remote authority.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] if remoting is not currently
  /// running, or [`RemotingError::TransportUnavailable`] if the quarantine
  /// signal could not be propagated through the transport.
  fn quarantine(&mut self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError>;

  /// Returns the local addresses this remoting instance advertises.
  fn addresses(&self) -> &[Address];
}
