//! Stores the association state and deferred queues for each authority.

use alloc::{collections::BTreeMap, string::{String, ToString}, vec::Vec};

use crate::core::{association_state::AssociationState, deferred_envelope::DeferredEnvelope};

/// Registry tracking per-authority state transitions and deferred queues.
#[derive(Default)]
pub struct EndpointRegistry {
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
  pub fn ensure_entry(&mut self, authority: &str) {
    self.entries.entry(authority.to_string()).or_insert_with(EndpointEntry::new);
  }

  /// Returns the current association state.
  pub fn state(&self, authority: &str) -> Option<&AssociationState> {
    self.entries.get(authority).map(|entry| &entry.state)
  }

  /// Sets the new association state and records metadata.
  pub fn set_state(&mut self, authority: &str, state: AssociationState, now: u64, reason: Option<&str>) {
    let entry = self.entries.entry(authority.to_string()).or_insert_with(EndpointEntry::new);
    entry.state = state;
    entry.last_change_ticks = now;
    entry.last_reason = reason.map(|text| text.to_string());
  }

  /// Pushes a deferred envelope for the given authority.
  pub fn push_deferred(&mut self, authority: &str, envelope: DeferredEnvelope) {
    self.entries.entry(authority.to_string()).or_insert_with(EndpointEntry::new).deferred.push(envelope);
  }

  /// Drains the deferred queue for the given authority.
  pub fn drain_deferred(&mut self, authority: &str) -> Vec<DeferredEnvelope> {
    self.entries.entry(authority.to_string()).or_insert_with(EndpointEntry::new).drain_deferred()
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
