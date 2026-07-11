//! Errors that can occur while spawning a new actor.

#[cfg(test)]
#[path = "spawn_error_test.rs"]
mod tests;

extern crate alloc;

use alloc::string::String;
use core::{
  error::Error,
  fmt::{Display, Formatter, Result as FmtResult},
};

use crate::system::ActorSystemBuildError;

/// Enumeration describing spawn failures.
#[derive(Debug)]
pub enum SpawnError {
  /// The requested name conflicts with an existing actor within the same scope.
  NameConflict(String),
  /// The actor system is shutting down or unavailable.
  SystemUnavailable,
  /// The actor system has not completed bootstrap (guardians not ready).
  SystemNotBootstrapped,
  /// The provided props were invalid for this actor system.
  InvalidProps(String),
  /// A `PinnedDispatcher` is already owned by another actor and cannot accept the new request.
  DispatcherAlreadyOwned,
  /// Actor system build error occurred during initialization.
  SystemBuildError(String),
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

  /// Creates a not-bootstrapped error.
  #[must_use]
  pub const fn system_not_bootstrapped() -> Self {
    Self::SystemNotBootstrapped
  }

  /// Creates an invalid props error with the provided reason.
  #[must_use]
  pub fn invalid_props(reason: impl Into<String>) -> Self {
    Self::InvalidProps(reason.into())
  }

  /// Creates a system build error from ActorSystemBuildError.
  #[must_use]
  pub fn from_actor_system_build_error(error: &ActorSystemBuildError) -> Self {
    Self::SystemBuildError(alloc::format!("{:?}", error))
  }
}

impl Display for SpawnError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | Self::NameConflict(name) => write!(f, "actor name conflict: {name}"),
      | Self::SystemUnavailable => f.write_str("actor system unavailable"),
      | Self::SystemNotBootstrapped => f.write_str("actor system not bootstrapped"),
      | Self::InvalidProps(reason) => write!(f, "invalid actor props: {reason}"),
      | Self::DispatcherAlreadyOwned => f.write_str("pinned dispatcher already owned"),
      | Self::SystemBuildError(reason) => write!(f, "actor system build failed: {reason}"),
    }
  }
}

impl Error for SpawnError {}
