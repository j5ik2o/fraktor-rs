//! Remote authority state management and quarantining.

#[cfg(test)]
mod tests;

use alloc::{
  collections::VecDeque,
  string::{String, ToString},
  vec::Vec,
};
use core::{marker::PhantomData, time::Duration};

use ahash::RandomState;
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex},
  sync::ArcShared,
};
use hashbrown::HashMap;

use crate::core::{
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
  entries: HashMap<String, AuthorityEntry<TB>, RandomState>,
  _marker: PhantomData<TB>,
}

/// Type alias using the default toolbox.
pub type RemoteAuthorityManager = RemoteAuthorityManagerGeneric<NoStdToolbox>;
/// Shared manager guarded by toolbox mutex.
pub type RemoteAuthorityManagerShared<TB> = ArcShared<ToolboxMutex<RemoteAuthorityManagerGeneric<TB>, TB>>;

impl<TB: RuntimeToolbox + 'static> RemoteAuthorityManagerGeneric<TB> {
  /// Creates a new manager with no authorities.
  #[must_use]
  pub fn new() -> Self {
    Self { entries: HashMap::with_hasher(RandomState::new()), _marker: PhantomData }
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
  /// Returns [`RemoteAuthorityError::Quarantined`] if the authority is quarantined.
  pub fn defer_send(
    &mut self,
    authority: impl Into<String>,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), RemoteAuthorityError> {
    self.defer_or_reject(authority.into(), message)
  }

  /// Tries to defer a message, returning an error if the authority is quarantined.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteAuthorityError::Quarantined`] if the authority is quarantined.
  pub fn try_defer_send(
    &mut self,
    authority: impl Into<String>,
    message: AnyMessageGeneric<TB>,
  ) -> Result<(), RemoteAuthorityError> {
    self.defer_or_reject(authority.into(), message)
  }

  fn defer_or_reject(&mut self, authority: String, message: AnyMessageGeneric<TB>) -> Result<(), RemoteAuthorityError> {
    let entry = self.entries.entry(authority).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));

    // Quarantine中は拒否
    if matches!(entry.state, AuthorityState::Quarantine { .. }) {
      return Err(RemoteAuthorityError::Quarantined);
    }

    entry.deferred.push_back(message);
    Ok(())
  }

  /// Transitions an authority to Connected and returns deferred messages.
  pub fn set_connected(&mut self, authority: &str) -> Option<VecDeque<AnyMessageGeneric<TB>>> {
    let entry =
      self.entries.entry(authority.to_string()).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));
    entry.state = AuthorityState::Connected;
    Some(core::mem::take(&mut entry.deferred))
  }

  /// Transitions an authority to Quarantine and discards deferred messages.
  pub fn set_quarantine(&mut self, authority: impl Into<String>, now: u64, duration: Option<Duration>) {
    let authority = authority.into();
    let deadline = duration.map(|d| now + d.as_secs());
    let entry = self.entries.entry(authority).or_insert_with(|| AuthorityEntry::new(AuthorityState::Unresolved));
    entry.state = AuthorityState::Quarantine { deadline };
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
}

impl<TB: RuntimeToolbox + 'static> Default for RemoteAuthorityManagerGeneric<TB> {
  fn default() -> Self {
    Self::new()
  }
}
