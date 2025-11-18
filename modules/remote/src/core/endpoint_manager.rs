//! Manages association state and deferred queues for remote endpoints.

use alloc::{string::String, vec, vec::Vec};

use fraktor_utils_rs::core::{runtime_toolbox::NoStdMutex, sync::ArcShared};

use crate::core::{
  association_state::AssociationState, deferred_envelope::DeferredEnvelope, endpoint_registry::EndpointRegistry,
  quarantine_reason::QuarantineReason, remote_node_id::RemoteNodeId, transport::TransportEndpoint,
};

/// Tracks per-authority association state.
pub struct EndpointManager {
  registry: ArcShared<NoStdMutex<EndpointRegistry>>,
}

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
    envelope:  DeferredEnvelope,
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

/// Effects emitted after processing a command.
#[derive(Debug, PartialEq, Eq)]
pub enum EndpointManagerEffect {
  /// Requests that a handshake frame be sent via the transport.
  StartHandshake {
    /// Authority that should start a handshake.
    authority: String,
    /// Endpoint to contact.
    endpoint:  TransportEndpoint,
  },
  /// Requests the consumer to deliver the provided envelopes.
  DeliverEnvelopes {
    /// Authority whose queue was flushed.
    authority: String,
    /// Envelopes to deliver in order.
    envelopes: Vec<DeferredEnvelope>,
  },
  /// Notifies that deferred envelopes were discarded due to quarantine.
  DiscardDeferred {
    /// Authority whose queue was discarded.
    authority: String,
    /// Reason associated with the discard.
    reason:    QuarantineReason,
    /// Envelopes that were dropped.
    envelopes: Vec<DeferredEnvelope>,
  },
}

/// Result returned after handling a command.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EndpointManagerResult {
  /// Side effects produced while handling the command.
  pub effects: Vec<EndpointManagerEffect>,
}

impl EndpointManager {
  /// Creates a new endpoint manager instance.
  #[must_use]
  pub fn new() -> Self {
    Self { registry: ArcShared::new(NoStdMutex::new(EndpointRegistry::default())) }
  }

  /// Returns the current association state for the provided authority when available.
  #[must_use]
  pub fn state(&self, authority: &str) -> Option<AssociationState> {
    let registry = self.registry.lock();
    registry.state(authority).cloned()
  }

  /// Handles a command and returns the produced effects.
  pub fn handle(&self, command: EndpointManagerCommand) -> EndpointManagerResult {
    match command {
      | EndpointManagerCommand::RegisterInbound { authority, now } => {
        let mut registry = self.registry.lock();
        registry.ensure_entry(&authority);
        registry.set_state(&authority, AssociationState::Unassociated, now, None);
        EndpointManagerResult::default()
      },
      | EndpointManagerCommand::Associate { authority, endpoint, now } => {
        let mut registry = self.registry.lock();
        registry.ensure_entry(&authority);
        registry.set_state(
          &authority,
          AssociationState::Associating { endpoint: endpoint.clone() },
          now,
          Some("associating"),
        );
        EndpointManagerResult { effects: vec![EndpointManagerEffect::StartHandshake { authority, endpoint }] }
      },
      | EndpointManagerCommand::EnqueueDeferred { authority, envelope } => {
        let mut registry = self.registry.lock();
        if matches!(registry.state(&authority), Some(AssociationState::Connected { .. })) {
          return EndpointManagerResult {
            effects: vec![EndpointManagerEffect::DeliverEnvelopes { authority, envelopes: vec![envelope] }],
          };
        }
        registry.push_deferred(&authority, envelope);
        EndpointManagerResult::default()
      },
      | EndpointManagerCommand::HandshakeAccepted { authority, remote_node, now } => {
        let mut registry = self.registry.lock();
        registry.ensure_entry(&authority);
        registry.set_state(
          &authority,
          AssociationState::Connected { remote: remote_node.clone() },
          now,
          Some("connected"),
        );
        let envelopes = registry.drain_deferred(&authority);
        if envelopes.is_empty() {
          EndpointManagerResult::default()
        } else {
          EndpointManagerResult { effects: vec![EndpointManagerEffect::DeliverEnvelopes { authority, envelopes }] }
        }
      },
      | EndpointManagerCommand::Quarantine { authority, reason, resume_at, now } => {
        let mut registry = self.registry.lock();
        registry.ensure_entry(&authority);
        let envelopes = registry.drain_deferred(&authority);
        registry.set_state(
          &authority,
          AssociationState::Quarantined { reason: reason.clone(), resume_at },
          now,
          Some(reason.message()),
        );
        if envelopes.is_empty() {
          EndpointManagerResult::default()
        } else {
          EndpointManagerResult {
            effects: vec![EndpointManagerEffect::DiscardDeferred { authority, reason, envelopes }],
          }
        }
      },
      | EndpointManagerCommand::Gate { authority, resume_at, now } => {
        let mut registry = self.registry.lock();
        registry.ensure_entry(&authority);
        registry.set_state(&authority, AssociationState::Gated { resume_at }, now, Some("gated"));
        EndpointManagerResult::default()
      },
      | EndpointManagerCommand::Recover { authority, endpoint, now } => {
        let mut registry = self.registry.lock();
        registry.ensure_entry(&authority);
        match endpoint {
          | Some(endpoint) => {
            registry.set_state(
              &authority,
              AssociationState::Associating { endpoint: endpoint.clone() },
              now,
              Some("recovering"),
            );
            EndpointManagerResult { effects: vec![EndpointManagerEffect::StartHandshake { authority, endpoint }] }
          },
          | None => {
            registry.set_state(&authority, AssociationState::Unassociated, now, Some("recovered"));
            EndpointManagerResult::default()
          },
        }
      },
    }
  }
}

#[cfg(test)]
mod tests;
