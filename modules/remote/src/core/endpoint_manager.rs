//! Manages association state and deferred queues for remote endpoints.

#[cfg(test)]
mod tests;

use alloc::{string::ToString, vec, vec::Vec};
use core::sync::atomic::Ordering;

use fraktor_actor_rs::core::event_stream::{CorrelationId, RemotingLifecycleEvent};
use portable_atomic::AtomicU64;

use crate::core::{
  association_state::AssociationState, endpoint_manager_command::EndpointManagerCommand,
  endpoint_manager_effect::EndpointManagerEffect, endpoint_manager_result::EndpointManagerResult,
  endpoint_registry::EndpointRegistry,
};

/// Tracks per-authority association state.
///
/// # Interior Mutability Removed
///
/// This type now requires `&mut self` for state-mutating operations.
/// Callers requiring shared access should use [`EndpointManagerShared`].
pub struct EndpointManager {
  registry:        EndpointRegistry,
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
  pub fn handle(&mut self, command: EndpointManagerCommand) -> EndpointManagerResult {
    match command {
      | EndpointManagerCommand::RegisterInbound { authority, now } => {
        self.registry.ensure_entry(&authority);
        self.registry.set_state(&authority, AssociationState::Unassociated, now, None);
        EndpointManagerResult::default()
      },
      | EndpointManagerCommand::Associate { authority, endpoint, now } => {
        self.registry.ensure_entry(&authority);
        self.registry.set_state(
          &authority,
          AssociationState::Associating { endpoint: endpoint.clone() },
          now,
          Some("associating"),
        );
        EndpointManagerResult { effects: vec![EndpointManagerEffect::StartHandshake { authority, endpoint }] }
      },
      | EndpointManagerCommand::EnqueueDeferred { authority, envelope } => {
        let envelope = *envelope;
        if matches!(self.registry.state(&authority), Some(AssociationState::Connected { .. })) {
          return EndpointManagerResult {
            effects: vec![EndpointManagerEffect::DeliverEnvelopes { authority, envelopes: vec![envelope] }],
          };
        }
        self.registry.push_deferred(&authority, envelope);
        EndpointManagerResult::default()
      },
      | EndpointManagerCommand::HandshakeAccepted { authority, remote_node, now } => {
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
        self.registry.ensure_entry(&authority);
        self.registry.set_state(&authority, AssociationState::Gated { resume_at }, now, Some("gated"));
        let correlation_id = self.next_correlation_id();
        EndpointManagerResult {
          effects: vec![EndpointManagerEffect::Lifecycle(RemotingLifecycleEvent::Gated { authority, correlation_id })],
        }
      },
      | EndpointManagerCommand::Recover { authority, endpoint, now } => {
        self.registry.ensure_entry(&authority);
        match endpoint {
          | Some(endpoint) => {
            self.registry.set_state(
              &authority,
              AssociationState::Associating { endpoint: endpoint.clone() },
              now,
              Some("recovering"),
            );
            EndpointManagerResult { effects: vec![EndpointManagerEffect::StartHandshake { authority, endpoint }] }
          },
          | None => {
            self.registry.set_state(&authority, AssociationState::Unassociated, now, Some("recovered"));
            EndpointManagerResult::default()
          },
        }
      },
    }
  }
}

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::sync_mutex_like::SyncMutexLike,
};

/// Shared wrapper for [`EndpointManager`] enabling interior mutability.
///
/// This wrapper provides `&self` methods that internally lock the underlying
/// [`EndpointManager`], allowing safe concurrent access from multiple owners.
pub struct EndpointManagerSharedGeneric<TB: RuntimeToolbox + 'static> {
  inner: ToolboxMutex<EndpointManager, TB>,
}

impl<TB: RuntimeToolbox + 'static> Default for EndpointManagerSharedGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}

impl<TB: RuntimeToolbox + 'static> EndpointManagerSharedGeneric<TB> {
  /// Creates a new shared endpoint manager instance.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: <TB::MutexFamily as SyncMutexFamily>::create(EndpointManager::new()) }
  }

  /// Returns the current association state for the provided authority when available.
  #[must_use]
  pub fn state(&self, authority: &str) -> Option<AssociationState> {
    self.inner.lock().state(authority)
  }

  /// Handles a command and returns the produced effects.
  pub fn handle(&self, command: EndpointManagerCommand) -> EndpointManagerResult {
    self.inner.lock().handle(command)
  }
}

/// Type alias for [`EndpointManagerSharedGeneric`] using the default [`NoStdToolbox`].
pub type EndpointManagerShared = EndpointManagerSharedGeneric<NoStdToolbox>;
