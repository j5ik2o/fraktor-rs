//! Errors that can occur while spawning a new actor.

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
  InvalidProps(&'static str),
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

  /// Creates an invalid props error with a static reason.
  #[must_use]
  pub const fn invalid_props(reason: &'static str) -> Self {
    Self::InvalidProps(reason)
  }
}
