//! Error types produced by the remoting foundation layer.

use alloc::string::{String, ToString};
use core::fmt;

use fraktor_actor_rs::core::{spawn::SpawnError, system::ActorSystemBuildError};

use crate::core::transport::TransportError;

/// Describes failures that can occur while configuring or operating remoting primitives.
#[derive(Debug)]
pub enum RemotingError {
  /// Remoting already entered the running state.
  AlreadyStarted,
  /// Remoting has not been started yet.
  NotStarted,
  /// A shutdown sequence is currently in progress.
  ShutdownInProgress,
  /// Remoting has already been shut down.
  AlreadyShutdown,
  /// System guardian references were unavailable during initialization.
  SystemGuardianUnavailable,
  /// Spawning the endpoint supervisor failed during initialization.
  EndpointSpawnFailed(SpawnError),
  /// Registering shutdown hooks with the system guardian failed.
  HookRegistrationFailed(String),
  /// Transport resolution failed during initialization.
  TransportUnavailable(String),
}

impl fmt::Display for RemotingError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::AlreadyStarted => write!(f, "remoting already started"),
      | Self::NotStarted => write!(f, "remoting has not started"),
      | Self::ShutdownInProgress => write!(f, "remoting shutdown in progress"),
      | Self::AlreadyShutdown => write!(f, "remoting already shut down"),
      | Self::SystemGuardianUnavailable => write!(f, "system guardian reference unavailable"),
      | Self::EndpointSpawnFailed(error) => write!(f, "failed to spawn endpoint supervisor: {error:?}"),
      | Self::HookRegistrationFailed(reason) => write!(f, "failed to register shutdown hook: {reason}"),
      | Self::TransportUnavailable(reason) => write!(f, "transport unavailable: {reason}"),
    }
  }
}

impl From<SpawnError> for RemotingError {
  fn from(value: SpawnError) -> Self {
    Self::EndpointSpawnFailed(value)
  }
}

impl From<RemotingError> for ActorSystemBuildError {
  fn from(value: RemotingError) -> Self {
    ActorSystemBuildError::Configuration(value.to_string())
  }
}

impl From<TransportError> for RemotingError {
  fn from(value: TransportError) -> Self {
    match value {
      | TransportError::UnsupportedScheme(scheme) => Self::TransportUnavailable(scheme),
      | other => Self::TransportUnavailable(other.to_string()),
    }
  }
}
