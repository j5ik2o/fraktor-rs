//! Remote authority state management and quarantining.

#[cfg(test)]
mod tests;

use alloc::{
  collections::VecDeque,
  string::{String, ToString},
};
use core::time::Duration;

use fraktor_utils_core_rs::{runtime_toolbox::SyncMutexFamily, sync::sync_mutex_like::SyncMutexLike};
use hashbrown::HashMap;

use crate::{
  NoStdToolbox, RuntimeToolbox, ToolboxMutex,
  messaging::AnyMessageGeneric,
  system::{AuthorityState, RemoteAuthorityError},
};

/// Entry tracking authority state and deferred messages.
#[derive(Debug)]
struct AuthorityEntry<TB: RuntimeToolbox + 'static> {
  state:    AuthorityState,
  deferred: VecDeque<AnyMessageGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> AuthorityEntry<TB> {
  const fn new(state: AuthorityState) -> Self {
    Self { state, deferred: VecDeque::new() }
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
    Self { entries: <TB::MutexFamily as SyncMutexFamily>::create(HashMap::new()) }
  }

  /// Returns the current state of an authority.
  #[must_use]
  pub fn state(&self, authority: &str) -> AuthorityState {
    self.entries.lock().get(authority).map(|e| e.state.clone()).unwrap_or(AuthorityState::Unresolved)
  }

  /// Marks an authority as unresolved and defers a message.
  pub fn defer_send(&self, authority: impl Into<String>, message: AnyMessageGeneric<TB>) {
    let authority = authority.into();
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));
    entry.deferred.push_back(message);
  }

  /// Tries to defer a message, returning an error if the authority is quarantined.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError::Quarantined`] if the authority is quarantined.
  pub fn try_defer_send(
    &self,
    authority: impl Into<String>,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), RemoteAuthorityError> {
    let authority = authority.into();
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));

    // Quarantine中は拒否
    if matches!(entry.state, AuthorityState::Quarantine { .. }) {
      return Err(RemoteAuthorityError::Quarantined);
    }

    entry.deferred.push_back(message);
    Ok(())
  }

  /// Transitions an authority to Connected and returns deferred messages.
  pub fn set_connected(&self, authority: &str) -> Option<VecDeque<AnyMessageGeneric<TB>>> {
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority.to_string()).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));
    entry.state = AuthorityState::Connected;
    Some(core::mem::take(&mut entry.deferred))
  }

  /// Transitions an authority to Quarantine and discards deferred messages.
  pub fn set_quarantine(&self, authority: impl Into<String>, now: u64, duration: Option<Duration>) {
    let authority = authority.into();
    let deadline = duration.map(|d| now + d.as_secs());
    let mut entries = self.entries.lock();
    let entry = entries.entry(authority).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));
    entry.state = AuthorityState::Quarantine { deadline };
    entry.deferred.clear();
  }

  /// Handles an InvalidAssociation event by transitioning to quarantine.
  pub fn handle_invalid_association(&self, authority: impl Into<String>, now: u64, duration: Option<Duration>) {
    self.set_quarantine(authority, now, duration);
  }

  /// Manually overrides quarantine and transitions to Connected.
  pub fn manual_override_to_connected(&self, authority: &str) {
    let mut entries = self.entries.lock();
    if let Some(entry) = entries.get_mut(authority) {
      entry.state = AuthorityState::Connected;
    }
  }

  /// Polls all authorities and lifts expired quarantines.
  ///
  /// # Arguments
  ///
  /// * `now` - Current monotonic time in seconds
  pub fn poll_quarantine_expiration(&self, now: u64) {
    let mut entries = self.entries.lock();
    for entry in entries.values_mut() {
      if let AuthorityState::Quarantine { deadline } = &entry.state
        && let Some(deadline_time) = deadline
        && now >= *deadline_time
      {
        entry.state = AuthorityState::Unresolved;
      }
    }
  }

  /// Transitions quarantine back to unresolved if quarantine period elapsed.
  pub fn lift_quarantine(&self, authority: &str) {
    let mut entries = self.entries.lock();
    if let Some(entry) = entries.get_mut(authority)
      && matches!(entry.state, AuthorityState::Quarantine { .. })
    {
      entry.state = AuthorityState::Unresolved;
    }
  }

  /// Returns count of deferred messages for an authority.
  #[must_use]
  pub fn deferred_count(&self, authority: &str) -> usize {
    self.entries.lock().get(authority).map(|e| e.deferred.len()).unwrap_or(0)
  }
}

impl<TB: RuntimeToolbox + 'static> Default for RemoteAuthorityManagerGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
