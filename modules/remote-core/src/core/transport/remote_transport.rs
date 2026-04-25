//! The single transport port the remote subsystem depends on.

use crate::core::{
  address::Address, association::QuarantineReason, envelope::OutboundEnvelope,
  transport::transport_error::TransportError,
};

/// The single transport port exposed by `fraktor-remote-core-rs`.
///
/// This trait mirrors Apache Pekko's `RemoteTransport` abstract class: it is the
/// only contract the higher-level state machines depend on, and every concrete
/// transport (TCP, loopback, in-process, ...) implements it on top of an
/// adapter crate (e.g. `fraktor-remote-adaptor-std-rs`).
///
/// ### Contract
///
/// - All methods are **synchronous** (`&self` for queries, `&mut self` for commands). No method is
///   `async`, returns a `Future`, or returns a lock guard type.
/// - Time input is **not** taken by the trait — the higher level passes time to its pure state
///   machines separately (see design decision 7).
/// - Errors are reported through [`TransportError`].
pub trait RemoteTransport {
  /// Starts the transport. Fails with [`TransportError::AlreadyRunning`] if
  /// the transport was already started.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::AlreadyRunning`] if called after a previous
  /// successful `start`, or a transport-specific variant if the listener
  /// could not be bound.
  fn start(&mut self) -> Result<(), TransportError>;

  /// Shuts the transport down. Fails with [`TransportError::NotStarted`] if
  /// called before `start`.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] if the transport was never
  /// successfully started.
  fn shutdown(&mut self) -> Result<(), TransportError>;

  /// Hands an [`OutboundEnvelope`] to the transport for delivery.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::SendFailed`] if the transport could not hand
  /// the envelope to the peer, [`TransportError::ConnectionClosed`] if the
  /// underlying channel has been closed, or [`TransportError::NotStarted`]
  /// if called before `start`.
  fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), TransportError>;

  /// Returns all addresses this transport currently advertises.
  fn addresses(&self) -> &[Address];

  /// Returns the canonical address clients should connect to.
  fn default_address(&self) -> Option<&Address>;

  /// Returns the local address used when talking to the given remote.
  fn local_address_for_remote(&self, remote: &Address) -> Option<&Address>;

  /// Quarantines the given remote authority.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] if the transport is not running,
  /// or a transport-specific variant if the quarantine signal could not be
  /// delivered.
  fn quarantine(&mut self, address: &Address, uid: Option<u64>, reason: QuarantineReason)
  -> Result<(), TransportError>;
}
