//! Errors that can occur while spawning a new actor.

#[cfg(test)]
mod tests;

extern crate alloc;

use alloc::string::String;

/// Enumeration describing spawn failures.
#[derive(Debug)]
pub enum SpawnError {
  /// The requested name conflicts with an existing actor within the same scope.
  NameConflict(String),
  /// The actor system is shutting down or unavailable.
  SystemUnavailable,
  /// The provided props were invalid for this actor system.
  InvalidProps(String),
  /// The mailbox configuration is incompatible with the dispatcher executor.
  ///
  /// This typically occurs when
  /// [`MailboxOverflowStrategy::Block`](crate::mailbox::MailboxOverflowStrategy::Block)
  /// is used with a single-threaded executor that doesn't support blocking operations.
  InvalidMailboxConfig(String),
}

impl SpawnError {
  /// Creates a name conflict error.
  #[must_use]
  pub fn name_conflict(name: impl Into<String>) -> Self {
    Self::NameConflict(name.into())
  }

  /// Creates a system unavailable error.
  #[must_use]
  pub const fn system_unavailable() -> Self {
    Self::SystemUnavailable
  }

  /// Creates an invalid props error with the provided reason.
  #[must_use]
  pub fn invalid_props(reason: impl Into<String>) -> Self {
    Self::InvalidProps(reason.into())
  }

  /// Creates an invalid mailbox configuration error with the provided reason.
  #[must_use]
  pub fn invalid_mailbox_config(reason: impl Into<String>) -> Self {
    Self::InvalidMailboxConfig(reason.into())
  }
}
