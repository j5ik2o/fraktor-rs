//! Commands accepted by the endpoint manager FSM.

use alloc::string::String;

use crate::core::{
  deferred_envelope::DeferredEnvelope, quarantine_reason::QuarantineReason, remote_node_id::RemoteNodeId,
  transport::TransportEndpoint,
};

/// Commands accepted by the endpoint manager FSM.
#[derive(Debug, PartialEq, Eq)]
pub enum EndpointManagerCommand {
  /// Registers a listener for the provided authority.
  RegisterInbound {
    /// Authority identifier for the newly registered listener.
    authority: String,
    /// Timestamp (monotonic ticks) of the event.
    now:       u64,
  },
  /// Initiates a handshake with the remote endpoint.
  Associate {
    /// Authority initiating the handshake.
    authority: String,
    /// Transport endpoint describing the remote authority.
    endpoint:  TransportEndpoint,
    /// Timestamp (monotonic ticks) of the event.
    now:       u64,
  },
  /// Enqueues an outbound envelope while the authority is not connected.
  EnqueueDeferred {
    /// Authority whose queue receives the envelope.
    authority: String,
    /// Envelope waiting for the association to complete.
    envelope:  alloc::boxed::Box<DeferredEnvelope>,
  },
  /// Marks the handshake as completed and stores the remote node identity.
  HandshakeAccepted {
    /// Authority transitioning to the connected state.
    authority:   String,
    /// Remote node identifier confirmed during handshake.
    remote_node: RemoteNodeId,
    /// Timestamp (monotonic ticks) of the event.
    now:         u64,
  },
  /// Forces the authority into a quarantined state and discards queued envelopes.
  Quarantine {
    /// Target authority to quarantine.
    authority: String,
    /// Describes why the quarantine was triggered.
    reason:    QuarantineReason,
    /// Optional deadline when the quarantine can be lifted.
    resume_at: Option<u64>,
    /// Timestamp when the quarantine was instituted.
    now:       u64,
  },
  /// Temporarily gates the authority without discarding envelopes.
  Gate {
    /// Target authority to gate.
    authority: String,
    /// Optional deadline when gating can be lifted.
    resume_at: Option<u64>,
    /// Timestamp when gating occurred.
    now:       u64,
  },
  /// Recovers a gated/quarantined authority and optionally restarts the handshake.
  Recover {
    /// Target authority to recover.
    authority: String,
    /// Optional endpoint to immediately re-handshake.
    endpoint:  Option<TransportEndpoint>,
    /// Timestamp of the recovery event.
    now:       u64,
  },
}
