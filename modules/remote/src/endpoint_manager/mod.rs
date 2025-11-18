//! Endpoint manager state machine for remote authorities.

mod association_state;
mod endpoint_manager_command;
mod remote_node_id;
#[cfg(test)]
mod tests;

use alloc::{collections::{BTreeMap, VecDeque}, string::{String, ToString}, vec::Vec};
use core::time::Duration;

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex};

pub use association_state::AssociationState;
pub use endpoint_manager_command::EndpointManagerCommand;
pub use remote_node_id::RemoteNodeId;

struct AuthorityEntry {
  state:    AssociationState,
  deferred: VecDeque<Vec<u8>>,
  last_change: u64,
  last_reason: Option<String>,
}

impl AuthorityEntry {
  fn new() -> Self {
    Self { state: AssociationState::Unassociated, deferred: VecDeque::new(), last_change: 0, last_reason: None }
  }
}

/// Snapshot of authority state for observability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndpointSnapshot {
  authority:  String,
  state:      AssociationState,
  last_change: u64,
  last_reason: Option<String>,
  deferred:   usize,
}

impl EndpointSnapshot {
  /// Returns authority identifier.
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns association state.
  #[must_use]
  pub fn state(&self) -> &AssociationState {
    &self.state
  }

  /// Returns monotonic timestamp of the last change.
  #[must_use]
  pub const fn last_change(&self) -> u64 {
    self.last_change
  }

  /// Returns last transition reason when available.
  #[must_use]
  pub fn last_reason(&self) -> Option<&str> {
    self.last_reason.as_deref()
  }

  /// Returns number of deferred messages.
  #[must_use]
  pub const fn deferred(&self) -> usize {
    self.deferred
  }
}

/// Describes why an authority was quarantined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuarantineReason {
  /// Remote UID mismatch was detected during handshake.
  UidMismatch,
  /// Manual quarantine triggered by operators.
  Manual(String),
}

/// Manages association/handshake state per remote authority.
pub struct EndpointManager {
  entries: ToolboxMutex<BTreeMap<String, AuthorityEntry>, NoStdToolbox>,
}

impl EndpointManager {
  /// Creates a new endpoint manager.
  #[must_use]
  pub fn new() -> Self {
    Self {
      entries: <<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(BTreeMap::new()),
    }
  }

  /// Returns current state for the authority.
  #[must_use]
  pub fn state(&self, authority: &str) -> AssociationState {
    self.entries.lock().get(authority).map(|entry| entry.state.clone()).unwrap_or(AssociationState::Unassociated)
  }

  /// Returns a snapshot of all known authorities.
  #[must_use]
  pub fn snapshots(&self) -> Vec<EndpointSnapshot> {
    self
      .entries
      .lock()
      .iter()
      .map(|(authority, entry)| EndpointSnapshot {
        authority:  authority.clone(),
        state:      entry.state.clone(),
        last_change: entry.last_change,
        last_reason: entry.last_reason.clone(),
        deferred:   entry.deferred.len(),
      })
      .collect()
  }

  /// Increments the handshake attempt counter.
  pub fn start_association(&self, authority: &str, now: u64) -> AssociationState {
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority.to_string()).or_insert_with(AuthorityEntry::new);
    entry.state = match entry.state {
      | AssociationState::Associating { attempt } => AssociationState::Associating { attempt: attempt + 1 },
      | _ => AssociationState::Associating { attempt: 1 },
    };
    entry.last_change = now;
    entry.last_reason = None;
    entry.state.clone()
  }

  /// Adds a payload to the deferred queue.
  pub fn defer_message(&self, authority: &str, payload: Vec<u8>) {
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority.to_string()).or_insert_with(AuthorityEntry::new);
    entry.deferred.push_back(payload);
  }

  /// Completes the handshake and returns deferred payloads.
  pub fn complete_handshake(&self, authority: &str, remote: RemoteNodeId, now: u64) -> Vec<Vec<u8>> {
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority.to_string()).or_insert_with(AuthorityEntry::new);
    entry.state = AssociationState::Connected { remote };
    entry.last_change = now;
    entry.last_reason = None;
    entry.deferred.drain(..).collect()
  }

  /// Marks an authority as quarantined and clears deferred messages.
  pub fn set_quarantine(
    &self,
    authority: &str,
    reason: QuarantineReason,
    since: u64,
    deadline: Option<Duration>,
  ) {
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority.to_string()).or_insert_with(AuthorityEntry::new);
    let deadline_secs = deadline.map(|dur| since + dur.as_secs());
    let description = match &reason {
      | QuarantineReason::UidMismatch => "uid mismatch".to_string(),
      | QuarantineReason::Manual(detail) => detail.clone(),
    };
    entry.state = AssociationState::Quarantined { reason: description.clone(), since, deadline: deadline_secs };
    entry.last_change = since;
    entry.last_reason = Some(description);
    entry.deferred.clear();
  }

  /// Manually overrides quarantine and transitions to Connected.
  pub fn manual_override_to_connected(
    &self,
    authority: &str,
    remote: RemoteNodeId,
    now: u64,
  ) -> Vec<Vec<u8>> {
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority.to_string()).or_insert_with(AuthorityEntry::new);
    entry.state = AssociationState::Connected { remote };
    entry.last_change = now;
    entry.last_reason = None;
    entry.deferred.drain(..).collect()
  }
}
