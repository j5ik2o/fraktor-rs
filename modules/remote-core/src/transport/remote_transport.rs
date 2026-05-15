//! The single transport port the remote subsystem depends on.

use alloc::boxed::Box;
use core::time::Duration;

use crate::{
  address::Address,
  association::QuarantineReason,
  envelope::OutboundEnvelope,
  transport::{transport_endpoint::TransportEndpoint, transport_error::TransportError},
  wire::{AckPdu, ControlPdu, HandshakePdu, RemoteDeploymentPdu},
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

  /// Establishes a peer writer for a remote address.
  ///
  /// Implementations that do not own connection state may return
  /// [`TransportError::NotAvailable`]. Delivery through [`Self::send`] remains
  /// synchronous; callers that use connection-oriented transports must
  /// establish peers before expecting envelope sends to succeed.
  ///
  /// Implementations must not perform long blocking socket I/O in this method:
  /// `Remote::run` may call it from a runtime task while applying association
  /// effects. Connection-oriented adapters should register a writer and drive
  /// the physical connect in their own runtime task.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] if the transport is not running,
  /// [`TransportError::SendFailed`] if the connection cannot be established,
  /// or [`TransportError::NotAvailable`] when unsupported by the transport.
  fn connect_peer(&mut self, remote: &Address) -> Result<(), TransportError> {
    let _ = remote;
    Err(TransportError::NotAvailable)
  }

  /// Hands an [`OutboundEnvelope`] to the transport for delivery.
  ///
  /// On failure the envelope is returned (boxed, matching `RemoteEvent::OutboundEnqueued`)
  /// alongside the error so the caller can re-enqueue it for retry without paying for a
  /// defensive clone on the hot success path. The `Box` keeps the `Err` variant small
  /// enough for `clippy::result_large_err`.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::SendFailed`] if the transport could not hand
  /// the envelope to the peer, [`TransportError::Backpressure`] if the
  /// transport's synchronous handoff queue is full,
  /// [`TransportError::ConnectionClosed`] if the underlying channel has been
  /// closed, or [`TransportError::NotStarted`] if called before `start`.
  fn send(&mut self, envelope: OutboundEnvelope) -> Result<(), (TransportError, Box<OutboundEnvelope>)>;

  /// Sends a wire-level control PDU to `remote`.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] if the transport is not running,
  /// [`TransportError::Backpressure`] if the transport's synchronous handoff
  /// queue is full, [`TransportError::ConnectionClosed`] if no connection to
  /// `remote` exists, or another transport-specific error when delivery fails.
  fn send_control(&mut self, remote: &Address, pdu: ControlPdu) -> Result<(), TransportError>;

  /// Sends a wire-level remote deployment PDU to `remote`.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] if the transport is not running,
  /// [`TransportError::Backpressure`] if the transport's synchronous handoff
  /// queue is full, or [`TransportError::ConnectionClosed`] if no connection to
  /// `remote` exists.
  fn send_deployment(&mut self, remote: &Address, pdu: RemoteDeploymentPdu) -> Result<(), TransportError> {
    let _ = remote;
    let _ = pdu;
    Err(TransportError::NotAvailable)
  }

  /// Sends a wire-level flush request to a specific outbound writer lane.
  ///
  /// Implementations without lane-aware writers may fall back to
  /// [`Self::send_control`]. Lane-aware transports must enqueue the request
  /// behind frames already queued for `lane_id`.
  ///
  /// # Errors
  ///
  /// Returns the same transport-level errors as [`Self::send_control`].
  fn send_flush_request(&mut self, remote: &Address, pdu: ControlPdu, lane_id: u32) -> Result<(), TransportError> {
    let _ = lane_id;
    self.send_control(remote, pdu)
  }

  /// Sends a wire-level ACK/NACK PDU to `remote`.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] if the transport is not running,
  /// [`TransportError::Backpressure`] if the transport's synchronous handoff
  /// queue is full, [`TransportError::ConnectionClosed`] if no connection to
  /// `remote` exists, or another transport-specific error when delivery fails.
  fn send_ack(&mut self, remote: &Address, pdu: AckPdu) -> Result<(), TransportError>;

  /// Sends a wire-level handshake PDU to `remote`.
  ///
  /// `Remote::run` calls this before [`Self::schedule_handshake_timeout`] when
  /// it executes `AssociationEffect::StartHandshake`.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] if the transport is not running,
  /// [`TransportError::Backpressure`] if the transport's synchronous handoff
  /// queue is full, [`TransportError::ConnectionClosed`] if no connection to
  /// `remote` exists, or another transport-specific error when delivery fails.
  fn send_handshake(&mut self, remote: &Address, pdu: HandshakePdu) -> Result<(), TransportError>;

  /// Schedules a generation-scoped handshake timeout for `authority`.
  ///
  /// Adapter implementations are responsible for pushing
  /// `RemoteEvent::HandshakeTimerFired { authority, generation, now_ms }`
  /// through their internal event sender when the timeout expires. `now_ms`
  /// must come from a monotonic clock owned by the adapter. `Remote::run`
  /// compares the event generation with the current association generation and
  /// discards stale timer events, so adapters do not need a cancellation API for
  /// superseded timers.
  ///
  /// # Errors
  ///
  /// Returns [`TransportError::NotStarted`] if the transport is not running, or
  /// another transport-specific error if the timer cannot be scheduled.
  fn schedule_handshake_timeout(
    &mut self,
    authority: &TransportEndpoint,
    timeout: Duration,
    generation: u64,
  ) -> Result<(), TransportError>;

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
