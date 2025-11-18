//! Stores the association state and deferred queues for each authority.

use alloc::{
  collections::BTreeMap,
  string::{String, ToString},
  vec::Vec,
};

use fraktor_actor_rs::core::system::AuthorityState;

use crate::core::{
  association_state::AssociationState, deferred_envelope::DeferredEnvelope,
  remote_authority_snapshot::RemoteAuthoritySnapshot,
};

/// Registry tracking per-authority state transitions and deferred queues.
#[derive(Default)]
pub(crate) struct EndpointRegistry {
  entries: BTreeMap<String, EndpointEntry>,
}

struct EndpointEntry {
  state:             AssociationState,
  deferred:          Vec<DeferredEnvelope>,
  last_change_ticks: u64,
  last_reason:       Option<String>,
}

impl EndpointRegistry {
  /// Ensures that the authority entry exists.
  pub(crate) fn ensure_entry(&mut self, authority: &str) {
    self.entries.entry(authority.to_string()).or_insert_with(EndpointEntry::new);
  }

  /// Returns the current association state.
  pub(crate) fn state(&self, authority: &str) -> Option<&AssociationState> {
    self.entries.get(authority).map(|entry| &entry.state)
  }

  /// Sets the new association state and records metadata.
  pub(crate) fn set_state(&mut self, authority: &str, state: AssociationState, now: u64, reason: Option<&str>) {
    let entry = self.entries.entry(authority.to_string()).or_insert_with(EndpointEntry::new);
    entry.state = state;
    entry.last_change_ticks = now;
    entry.last_reason = reason.map(|text| text.to_string());
  }

  /// Pushes a deferred envelope for the given authority.
  pub(crate) fn push_deferred(&mut self, authority: &str, envelope: DeferredEnvelope) {
    self.entries.entry(authority.to_string()).or_insert_with(EndpointEntry::new).deferred.push(envelope);
  }

  /// Drains the deferred queue for the given authority.
  pub(crate) fn drain_deferred(&mut self, authority: &str) -> Vec<DeferredEnvelope> {
    self.entries.entry(authority.to_string()).or_insert_with(EndpointEntry::new).drain_deferred()
  }

  /// Returns a snapshot of every tracked authority.
  #[allow(dead_code)]
  pub(crate) fn snapshots(&self) -> Vec<RemoteAuthoritySnapshot> {
    self
      .entries
      .iter()
      .map(|(authority, entry)| {
        RemoteAuthoritySnapshot::new(
          authority.clone(),
          map_state(&entry.state),
          entry.last_change_ticks,
          entry.deferred.len() as u32,
        )
      })
      .collect()
  }
}

#[allow(dead_code)]
fn map_state(state: &AssociationState) -> AuthorityState {
  match state {
    | AssociationState::Connected { .. } => AuthorityState::Connected,
    | AssociationState::Quarantined { resume_at, .. } | AssociationState::Gated { resume_at } => {
      AuthorityState::Quarantine { deadline: *resume_at }
    },
    | AssociationState::Unassociated | AssociationState::Associating { .. } => AuthorityState::Unresolved,
  }
}

impl EndpointEntry {
  fn new() -> Self {
    Self {
      state:             AssociationState::Unassociated,
      deferred:          Vec::new(),
      last_change_ticks: 0,
      last_reason:       None,
    }
  }

  fn drain_deferred(&mut self) -> Vec<DeferredEnvelope> {
    core::mem::take(&mut self.deferred)
  }
}
