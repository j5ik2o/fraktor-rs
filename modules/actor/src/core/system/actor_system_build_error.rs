use alloc::string::String;
use core::fmt;

use crate::core::{scheduler::TickDriverError, spawn::SpawnError};

/// Error emitted when [`ActorSystemBuilder`] fails to initialize the runtime.
#[derive(Debug)]
pub enum ActorSystemBuildError {
  /// Tick driver configuration was not provided.
  MissingTickDriver,
  /// Failed while spawning the actor system guardians.
  Spawn(SpawnError),
  /// Tick driver provisioning failed.
  TickDriver(TickDriverError),
  /// Builder-supplied configuration (extensions/providers) failed to install.
  Configuration(String),
}

impl fmt::Display for ActorSystemBuildError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::MissingTickDriver => write!(f, "tick driver configuration is required"),
      | Self::Spawn(err) => write!(f, "actor system bootstrap failed: {err:?}"),
      | Self::TickDriver(err) => write!(f, "tick driver provisioning failed: {err}"),
      | Self::Configuration(err) => write!(f, "builder configuration failed: {err}"),
    }
  }
}

impl From<SpawnError> for ActorSystemBuildError {
  fn from(value: SpawnError) -> Self {
    Self::Spawn(value)
  }
}

impl From<TickDriverError> for ActorSystemBuildError {
  fn from(value: TickDriverError) -> Self {
    Self::TickDriver(value)
  }
}
