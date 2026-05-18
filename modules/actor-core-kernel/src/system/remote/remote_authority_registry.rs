//! Remote authority state management and quarantining.

#[cfg(test)]
#[path = "remote_authority_registry_test.rs"]
mod tests;

use alloc::{
  collections::VecDeque,
  string::{String, ToString},
  vec::Vec,
};
use core::{marker::PhantomData, time::Duration};

use ahash::RandomState;
use hashbrown::HashMap;

use crate::{
  actor::messaging::AnyMessage,
  system::{remote::RemoteAuthorityError, state::AuthorityState},
};

const MAX_DEFERRED_MESSAGES_PER_AUTHORITY: usize = 1024;
const MAX_DEFERRED_MESSAGES_TOTAL: usize = 8192;

/// Entry tracking authority state and deferred messages.
#[derive(Debug)]
struct AuthorityEntry {
  state:    AuthorityState,
  deferred: VecDeque<AnyMessage>,
}

impl AuthorityEntry {
  const fn new(state: AuthorityState) -> Self {
    Self { state, deferred: VecDeque::new() }
  }
}

/// Tracks remote authority state transitions and deferred message queues.
pub struct RemoteAuthorityRegistry {
  entries:        HashMap<String, AuthorityEntry, RandomState>,
  total_deferred: usize,
  _marker:        PhantomData<()>,
}

impl RemoteAuthorityRegistry {
  /// Creates a new registry with no authorities.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: HashMap::with_hasher(RandomState::new()), total_deferred: 0, _marker: PhantomData }
  }

  /// Returns the current state of an authority.
  #[must_use]
  pub fn state(&self, authority: &str) -> AuthorityState {
    self.entries.get(authority).map(|e| e.state.clone()).unwrap_or(AuthorityState::Unresolved)
  }

  /// Marks an authority as unresolved and defers a message.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError::Quarantined`] if the authority is quarantined, or
  /// [`RemoteAuthorityError::DeferredQueueFull`] if the deferred queue reached its limit.
  pub fn defer_send(&mut self, authority: impl Into<String>, message: AnyMessage) -> Result<(), RemoteAuthorityError> {
    self.defer_or_reject(authority.into(), message)
  }

  /// Tries to defer a message, returning an error if the authority is quarantined.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError::Quarantined`] if the authority is quarantined, or
  /// [`RemoteAuthorityError::DeferredQueueFull`] if the deferred queue reached its limit.
  pub fn try_defer_send(
    &mut self,
    authority: impl Into<String>,
    message: AnyMessage,
  ) -> Result<(), RemoteAuthorityError> {
    self.defer_or_reject(authority.into(), message)
  }

  fn defer_or_reject(&mut self, authority: String, message: AnyMessage) -> Result<(), RemoteAuthorityError> {
    if let Some(entry) = self.entries.get_mut(&authority) {
      return Self::defer_into_entry(entry, &mut self.total_deferred, message);
    }

    let mut entry = AuthorityEntry::new(AuthorityState::Unresolved);
    Self::defer_into_entry(&mut entry, &mut self.total_deferred, message)?;
    self.entries.insert(authority, entry);
    Ok(())
  }

  fn defer_into_entry(
    entry: &mut AuthorityEntry,
    total_deferred: &mut usize,
    message: AnyMessage,
  ) -> Result<(), RemoteAuthorityError> {
    // Quarantine中は拒否
    if matches!(entry.state, AuthorityState::Quarantine { .. }) {
      return Err(RemoteAuthorityError::Quarantined);
    }

    if entry.deferred.len() >= MAX_DEFERRED_MESSAGES_PER_AUTHORITY || *total_deferred >= MAX_DEFERRED_MESSAGES_TOTAL {
      return Err(RemoteAuthorityError::DeferredQueueFull);
    }

    entry.deferred.push_back(message);
    *total_deferred += 1;
    Ok(())
  }

  /// Transitions an authority to Connected and returns deferred messages.
  pub fn set_connected(&mut self, authority: &str) -> Option<VecDeque<AnyMessage>> {
    let entry =
      self.entries.entry(authority.to_string()).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));
    entry.state = AuthorityState::Connected;
    self.total_deferred = self.total_deferred.saturating_sub(entry.deferred.len());
    Some(core::mem::take(&mut entry.deferred))
  }

  /// Transitions an authority to Quarantine and discards deferred messages.
  pub fn set_quarantine(&mut self, authority: impl Into<String>, now: u64, duration: Option<Duration>) {
    let authority = authority.into();
    let deadline = duration.map(|d| now + d.as_secs());
    let entry = self.entries.entry(authority).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));
    entry.state = AuthorityState::Quarantine { deadline };
    self.total_deferred = self.total_deferred.saturating_sub(entry.deferred.len());
    entry.deferred.clear();
  }

  /// Handles an InvalidAssociation event by transitioning to quarantine.
  pub fn handle_invalid_association(&mut self, authority: impl Into<String>, now: u64, duration: Option<Duration>) {
    self.set_quarantine(authority, now, duration);
  }

  /// Manually overrides quarantine and transitions to Connected.
  pub fn manual_override_to_connected(&mut self, authority: &str) {
    if let Some(entry) = self.entries.get_mut(authority) {
      entry.state = AuthorityState::Connected;
    }
  }

  /// Polls all authorities and lifts expired quarantines, returning the affected authorities.
  pub fn poll_quarantine_expiration(&mut self, now: u64) -> Vec<String> {
    let mut lifted = Vec::new();
    for (authority, entry) in self.entries.iter_mut() {
      if let AuthorityState::Quarantine { deadline } = &entry.state
        && let Some(deadline_time) = deadline
        && now >= *deadline_time
      {
        entry.state = AuthorityState::Unresolved;
        lifted.push(authority.clone());
      }
    }
    lifted
  }

  /// Transitions quarantine back to unresolved if quarantine period elapsed.
  pub fn lift_quarantine(&mut self, authority: &str) {
    if let Some(entry) = self.entries.get_mut(authority)
      && matches!(entry.state, AuthorityState::Quarantine { .. })
    {
      entry.state = AuthorityState::Unresolved;
    }
  }

  /// Returns a snapshot of all known authorities and their states.
  #[must_use]
  pub fn snapshots(&self) -> Vec<(String, AuthorityState)> {
    self.entries.iter().map(|(authority, entry)| (authority.clone(), entry.state.clone())).collect()
  }

  /// Returns count of deferred messages for an authority.
  #[must_use]
  pub fn deferred_count(&self, authority: &str) -> usize {
    self.entries.get(authority).map(|e| e.deferred.len()).unwrap_or(0)
  }

  #[cfg(test)]
  const fn total_deferred_count(&self) -> usize {
    self.total_deferred
  }
}

impl Default for RemoteAuthorityRegistry {
  fn default() -> Self {
    Self::new()
  }
}
