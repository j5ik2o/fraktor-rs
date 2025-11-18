//! Captures the current status of a remote authority.

use alloc::string::String;

use fraktor_actor_rs::core::system::AuthorityState;

/// Immutable view of an authority's state and queue depth.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteAuthoritySnapshot {
  authority:         String,
  state:             AuthorityState,
  last_change_ticks: u64,
  deferred_messages: u32,
}

impl RemoteAuthoritySnapshot {
  /// Creates a new snapshot for the provided authority.
  #[must_use]
  pub fn new(
    authority: impl Into<String>,
    state: AuthorityState,
    last_change_ticks: u64,
    deferred_messages: u32,
  ) -> Self {
    Self { authority: authority.into(), state, last_change_ticks, deferred_messages }
  }

  /// Returns the authority identifier (usually `host:port`).
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the recorded state.
  #[must_use]
  pub fn state(&self) -> &AuthorityState {
    &self.state
  }

  /// Returns the monotonic tick when the state last changed.
  #[must_use]
  pub const fn last_change_ticks(&self) -> u64 {
    self.last_change_ticks
  }

  /// Returns the number of deferred messages queued for the authority.
  #[must_use]
  pub const fn deferred_messages(&self) -> u32 {
    self.deferred_messages
  }
}
