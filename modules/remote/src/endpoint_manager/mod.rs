//! Endpoint manager state machine for remote authorities.

mod association_state;
mod endpoint_manager_command;
mod remote_node_id;
#[cfg(test)]
mod tests;

use alloc::{collections::{BTreeMap, VecDeque}, string::{String, ToString}, vec::Vec};

use fraktor_utils_rs::core::runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily, ToolboxMutex};

pub use association_state::AssociationState;
pub use endpoint_manager_command::EndpointManagerCommand;
pub use remote_node_id::RemoteNodeId;

struct AuthorityEntry {
  state:    AssociationState,
  deferred: VecDeque<Vec<u8>>,
}

impl AuthorityEntry {
  fn new() -> Self {
    Self { state: AssociationState::Unassociated, deferred: VecDeque::new() }
  }
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

  /// Increments the handshake attempt counter.
  pub fn start_association(&self, authority: &str) -> AssociationState {
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority.to_string()).or_insert_with(AuthorityEntry::new);
    entry.state = match entry.state {
      | AssociationState::Associating { attempt } => AssociationState::Associating { attempt: attempt + 1 },
      | _ => AssociationState::Associating { attempt: 1 },
    };
    entry.state.clone()
  }

  /// Adds a payload to the deferred queue.
  pub fn defer_message(&self, authority: &str, payload: Vec<u8>) {
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority.to_string()).or_insert_with(AuthorityEntry::new);
    entry.deferred.push_back(payload);
  }

  /// Completes the handshake and returns deferred payloads.
  pub fn complete_handshake(&self, authority: &str, remote: RemoteNodeId) -> Vec<Vec<u8>> {
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority.to_string()).or_insert_with(AuthorityEntry::new);
    entry.state = AssociationState::Connected { remote };
    entry.deferred.drain(..).collect()
  }
}
