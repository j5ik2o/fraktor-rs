//! Remote authority state transition event.

use alloc::string::String;

use crate::core::system::AuthorityState;

/// Event payload describing a remote authority state transition.
#[derive(Clone, Debug)]
pub struct RemoteAuthorityEvent {
  authority: String,
  state:     AuthorityState,
}

impl RemoteAuthorityEvent {
  /// Creates a new event for the specified authority/state combination.
  #[must_use]
  pub fn new(authority: impl Into<String>, state: AuthorityState) -> Self {
    Self { authority: authority.into(), state }
  }

  /// Returns the authority identifier (usually `host:port`).
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the recorded state.
  #[must_use]
  pub const fn state(&self) -> &AuthorityState {
    &self.state
  }
}
