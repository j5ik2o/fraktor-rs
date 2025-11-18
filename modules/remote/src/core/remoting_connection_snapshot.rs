//! Snapshot describing a remoting connection authority.

use alloc::string::String;

use fraktor_actor_rs::core::system::AuthorityState;

/// Immutable snapshot capturing authority state for observability APIs.
#[derive(Clone, Debug, PartialEq)]
pub struct RemotingConnectionSnapshot {
  authority: String,
  state:     AuthorityState,
}

impl RemotingConnectionSnapshot {
  /// Creates a new snapshot instance.
  #[must_use]
  pub fn new(authority: impl Into<String>, state: AuthorityState) -> Self {
    Self { authority: authority.into(), state }
  }

  /// Returns the authority identifier (usually `host:port`).
  #[must_use]
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the associated authority state.
  #[must_use]
  pub const fn state(&self) -> &AuthorityState {
    &self.state
  }
}
