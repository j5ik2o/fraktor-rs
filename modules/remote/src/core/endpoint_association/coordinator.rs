//! Coordinates association state and deferred queues for remote endpoints.

#[cfg(test)]
mod tests;

use alloc::{string::ToString, vec, vec::Vec};
use core::sync::atomic::Ordering;

use fraktor_actor_rs::core::event::stream::{CorrelationId, RemotingLifecycleEvent};
use portable_atomic::AtomicU64;

use super::{
  command::EndpointAssociationCommand, effect::EndpointAssociationEffect, result::EndpointAssociationResult,
  state::AssociationState,
};
use crate::core::endpoint_registry::EndpointRegistry;

/// Tracks per-authority association state.
///
/// # Interior Mutability Removed
///
/// This type now requires `&mut self` for state-mutating operations.
/// Callers requiring shared access should use [`EndpointAssociationCoordinatorShared`].
pub struct EndpointAssociationCoordinator {
  registry:        EndpointRegistry,
  correlation_seq: AtomicU64,
}

impl Default for EndpointAssociationCoordinator {
  fn default() -> Self {
    Self::new()
  }
}

impl EndpointAssociationCoordinator {
  /// Creates a new endpoint association coordinator instance.
  #[must_use]
  pub fn new() -> Self {
    Self { registry: EndpointRegistry::default(), correlation_seq: AtomicU64::new(1) }
  }

  /// Returns the current association state for the provided authority when available.
  #[must_use]
  pub fn state(&self, authority: &str) -> Option<AssociationState> {
    self.registry.state(authority).cloned()
  }

  fn next_correlation_id(&self) -> CorrelationId {
    let seq = self.correlation_seq.fetch_add(1, Ordering::Relaxed) as u128;
    CorrelationId::from_u128(seq)
  }

  /// Handles a command and returns the produced effects.
  pub fn handle(&mut self, command: EndpointAssociationCommand) -> EndpointAssociationResult {
    match command {
      | EndpointAssociationCommand::RegisterInbound { authority, now } => {
        self.registry.ensure_entry(&authority);
        self.registry.set_state(&authority, AssociationState::Unassociated, now, None);
        EndpointAssociationResult::default()
      },
      | EndpointAssociationCommand::Associate { authority, endpoint, now } => {
        self.registry.ensure_entry(&authority);
        self.registry.set_state(
          &authority,
          AssociationState::Associating { endpoint: endpoint.clone() },
          now,
          Some("associating"),
        );
        EndpointAssociationResult { effects: vec![EndpointAssociationEffect::StartHandshake { authority, endpoint }] }
      },
      | EndpointAssociationCommand::EnqueueDeferred { authority, envelope } => {
        let envelope = *envelope;
        if matches!(self.registry.state(&authority), Some(AssociationState::Connected { .. })) {
          return EndpointAssociationResult {
            effects: vec![EndpointAssociationEffect::DeliverEnvelopes { authority, envelopes: vec![envelope] }],
          };
        }
        self.registry.push_deferred(&authority, envelope);
        EndpointAssociationResult::default()
      },
      | EndpointAssociationCommand::HandshakeAccepted { authority, remote_node, now } => {
        self.registry.ensure_entry(&authority);
        self.registry.set_state(
          &authority,
          AssociationState::Connected { remote: remote_node.clone() },
          now,
          Some("connected"),
        );
        let envelopes = self.registry.drain_deferred(&authority);
        let mut effects = Vec::new();
        if !envelopes.is_empty() {
          effects.push(EndpointAssociationEffect::DeliverEnvelopes { authority: authority.clone(), envelopes });
        }
        let correlation_id = self.next_correlation_id();
        effects.push(EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Connected {
          authority,
          remote_system: remote_node.system().to_string(),
          remote_uid: remote_node.uid(),
          correlation_id,
        }));
        EndpointAssociationResult { effects }
      },
      | EndpointAssociationCommand::Quarantine { authority, reason, resume_at, now } => {
        self.registry.ensure_entry(&authority);
        let envelopes = self.registry.drain_deferred(&authority);
        self.registry.set_state(
          &authority,
          AssociationState::Quarantined { reason: reason.clone(), resume_at },
          now,
          Some(reason.message()),
        );
        let mut effects = Vec::new();
        if !envelopes.is_empty() {
          effects.push(EndpointAssociationEffect::DiscardDeferred {
            authority: authority.clone(),
            reason: reason.clone(),
            envelopes,
          });
        }
        let correlation_id = self.next_correlation_id();
        effects.push(EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Quarantined {
          authority,
          reason: reason.message().to_string(),
          correlation_id,
        }));
        EndpointAssociationResult { effects }
      },
      | EndpointAssociationCommand::Gate { authority, resume_at, now } => {
        self.registry.ensure_entry(&authority);
        self.registry.set_state(&authority, AssociationState::Gated { resume_at }, now, Some("gated"));
        let correlation_id = self.next_correlation_id();
        EndpointAssociationResult {
          effects: vec![EndpointAssociationEffect::Lifecycle(RemotingLifecycleEvent::Gated {
            authority,
            correlation_id,
          })],
        }
      },
      | EndpointAssociationCommand::Recover { authority, endpoint, now } => {
        self.registry.ensure_entry(&authority);
        match endpoint {
          | Some(endpoint) => {
            self.registry.set_state(
              &authority,
              AssociationState::Associating { endpoint: endpoint.clone() },
              now,
              Some("recovering"),
            );
            EndpointAssociationResult {
              effects: vec![EndpointAssociationEffect::StartHandshake { authority, endpoint }],
            }
          },
          | None => {
            self.registry.set_state(&authority, AssociationState::Unassociated, now, Some("recovered"));
            EndpointAssociationResult::default()
          },
        }
      },
    }
  }
}
