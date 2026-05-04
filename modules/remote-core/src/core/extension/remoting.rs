//! `Remoting` port trait: pure lifecycle surface of the remote subsystem.

use alloc::vec::Vec;

use crate::core::{address::Address, association::QuarantineReason, extension::remoting_error::RemotingError};

/// Shared lifecycle port for the remote subsystem.
///
/// This trait carries **only** the remote lifecycle responsibilities.
/// Implementations absorb the concurrency policy internally; callers use a
/// synchronous `&self` surface and do not receive lock guards or futures.
///
/// All methods are synchronous. No `async fn` and no `Future` return types.
pub trait Remoting {
  /// Starts the remote subsystem.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::AlreadyRunning`] if remoting is already
  /// running, or [`RemotingError::TransportUnavailable`] /
  /// [`RemotingError::InvalidTransition`] if the underlying transport
  /// could not be brought up.
  fn start(&self) -> Result<(), RemotingError>;

  /// Shuts the remote subsystem down.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] if remoting was never running.
  fn shutdown(&self) -> Result<(), RemotingError>;

  /// Quarantines the given remote authority.
  ///
  /// # Errors
  ///
  /// Returns [`RemotingError::NotStarted`] if remoting is not currently
  /// running, or [`RemotingError::TransportUnavailable`] if the quarantine
  /// signal could not be propagated through the transport.
  fn quarantine(&self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError>;

  /// Returns the local addresses this remoting instance advertises.
  fn addresses(&self) -> Vec<Address>;
}
