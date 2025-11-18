//! Manages association state and deferred queues for remote endpoints.

#[cfg(test)]
mod tests;

use alloc::{string::ToString, vec, vec::Vec};
use core::sync::atomic::Ordering;

use fraktor_actor_rs::core::event_stream::{CorrelationId, RemotingLifecycleEvent};
use fraktor_utils_rs::core::{runtime_toolbox::NoStdMutex, sync::ArcShared};
use portable_atomic::AtomicU64;

use crate::core::{
  association_state::AssociationState, endpoint_manager_command::EndpointManagerCommand,
  endpoint_manager_effect::EndpointManagerEffect, endpoint_manager_result::EndpointManagerResult,
  endpoint_registry::EndpointRegistry,
};

/// Tracks per-authority association state.
pub struct EndpointManager {
  registry:        ArcShared<NoStdMutex<EndpointRegistry>>,
  correlation_seq: AtomicU64,
}

impl Default for EndpointManager {
  fn default() -> Self {
    Self::new()
  }
}

impl EndpointManager {
  /// Creates a new endpoint manager instance.
  #[must_use]
  pub fn new() -> Self {
    Self {
      registry:        ArcShared::new(NoStdMutex::new(EndpointRegistry::default())),
      correlation_seq: AtomicU64::new(1),
    }
  }

  /// Returns the current association state for the provided authority when available.
  #[must_use]
  pub fn state(&self, authority: &str) -> Option<AssociationState> {
    let registry = self.registry.lock();
    registry.state(authority).cloned()
  }

  fn next_correlation_id(&self) -> CorrelationId {
    let seq = self.correlation_seq.fetch_add(1, Ordering::Relaxed) as u128;
    CorrelationId::from_u128(seq)
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
        let mut effects = Vec::new();
        if !envelopes.is_empty() {
          effects.push(EndpointManagerEffect::DeliverEnvelopes { authority: authority.clone(), envelopes });
        }
        let correlation_id = self.next_correlation_id();
        effects.push(EndpointManagerEffect::Lifecycle(RemotingLifecycleEvent::Connected {
          authority,
          remote_system: remote_node.system().to_string(),
          remote_uid: remote_node.uid(),
          correlation_id,
        }));
        EndpointManagerResult { effects }
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
        let mut effects = Vec::new();
        if !envelopes.is_empty() {
          effects.push(EndpointManagerEffect::DiscardDeferred {
            authority: authority.clone(),
            reason: reason.clone(),
            envelopes,
          });
        }
        let correlation_id = self.next_correlation_id();
        effects.push(EndpointManagerEffect::Lifecycle(RemotingLifecycleEvent::Quarantined {
          authority,
          reason: reason.message().to_string(),
          correlation_id,
        }));
        EndpointManagerResult { effects }
      },
      | EndpointManagerCommand::Gate { authority, resume_at, now } => {
        let mut registry = self.registry.lock();
        registry.ensure_entry(&authority);
        registry.set_state(&authority, AssociationState::Gated { resume_at }, now, Some("gated"));
        let correlation_id = self.next_correlation_id();
        EndpointManagerResult {
          effects: vec![EndpointManagerEffect::Lifecycle(RemotingLifecycleEvent::Gated { authority, correlation_id })],
        }
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
