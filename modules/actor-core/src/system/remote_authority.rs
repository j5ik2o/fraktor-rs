//! Remote authority state management and quarantining.

use alloc::{collections::VecDeque, string::String};
use core::time::Duration;

use fraktor_utils_core_rs::{runtime_toolbox::SyncMutexFamily, sync::sync_mutex_like::SyncMutexLike};
use hashbrown::HashMap;

use crate::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, messaging::AnyMessageGeneric};

#[cfg(test)]
mod tests;

/// State of a remote authority.
#[derive(Clone, Debug, PartialEq)]
pub enum AuthorityState {
  /// Authority has not been resolved yet; messages are deferred.
  Unresolved,
  /// Authority is connected and ready to accept messages.
  Connected,
  /// Authority is quarantined; new sends are rejected.
  Quarantine {
    /// Deadline when quarantine should be lifted.
    deadline: Option<Duration>,
  },
}

/// Entry tracking authority state and deferred messages.
#[derive(Debug)]
struct AuthorityEntry<TB: RuntimeToolbox + 'static> {
  state:    AuthorityState,
  deferred: VecDeque<AnyMessageGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> AuthorityEntry<TB> {
  fn new(state: AuthorityState) -> Self {
    Self {
      state,
      deferred: VecDeque::new(),
    }
  }
}

/// Manages remote authority state transitions and deferred message queues.
pub struct RemoteAuthorityManagerGeneric<TB: RuntimeToolbox + 'static> {
  entries: ToolboxMutex<HashMap<String, AuthorityEntry<TB>>, TB>,
}

/// Type alias using the default toolbox.
pub type RemoteAuthorityManager = RemoteAuthorityManagerGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> RemoteAuthorityManagerGeneric<TB> {
  /// Creates a new manager with no authorities.
  #[must_use]
  pub fn new() -> Self {
    Self {
      entries: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()),
    }
  }

  /// Returns the current state of an authority.
  #[must_use]
  pub fn state(&self, authority: &str) -> AuthorityState {
    self
      .entries
      .lock()
      .get(authority)
      .map(|e| e.state.clone())
      .unwrap_or(AuthorityState::Unresolved)
  }

  /// Marks an authority as unresolved and defers a message.
  pub fn defer_send(&self, authority: impl Into<String>, message: AnyMessageGeneric<TB>) {
    let authority = authority.into();
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));
    entry.deferred.push_back(message);
  }

  /// Transitions an authority to Connected and returns deferred messages.
  pub fn set_connected(&self, authority: &str) -> Option<VecDeque<AnyMessageGeneric<TB>>> {
    let mut entries = self.entries.lock();
    entries.get_mut(authority).map(|entry| {
      entry.state = AuthorityState::Connected;
      core::mem::take(&mut entry.deferred)
    })
  }

  /// Transitions an authority to Quarantine and discards deferred messages.
  pub fn set_quarantine(&self, authority: impl Into<String>, deadline: Option<Duration>) {
    let authority = authority.into();
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));
    entry.state = AuthorityState::Quarantine { deadline };
    entry.deferred.clear();
  }

  /// Transitions quarantine back to unresolved if quarantine period elapsed.
  pub fn lift_quarantine(&self, authority: &str) {
    let mut entries = self.entries.lock();
    if let Some(entry) = entries.get_mut(authority) {
      if matches!(entry.state, AuthorityState::Quarantine { .. }) {
        entry.state = AuthorityState::Unresolved;
      }
    }
  }

  /// Returns count of deferred messages for an authority.
  #[must_use]
  pub fn deferred_count(&self, authority: &str) -> usize {
    self
      .entries
      .lock()
      .get(authority)
      .map(|e| e.deferred.len())
      .unwrap_or(0)
  }
}

impl<TB: RuntimeToolbox + 'static> Default for RemoteAuthorityManagerGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
