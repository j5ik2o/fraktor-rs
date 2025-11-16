use core::fmt;

use crate::{scheduler::TickDriverError, spawn::SpawnError};

/// Error emitted when [`ActorSystemBuilder`] fails to initialize the runtime.
#[derive(Debug)]
pub enum ActorSystemBuildError {
  /// Tick driver configuration was not provided.
  MissingTickDriver,
  /// Failed while spawning the actor system guardians.
  Spawn(SpawnError),
  /// Tick driver provisioning failed.
  TickDriver(TickDriverError),
}

impl fmt::Display for ActorSystemBuildError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      | Self::MissingTickDriver => write!(f, "tick driver configuration is required"),
      | Self::Spawn(err) => write!(f, "actor system bootstrap failed: {err:?}"),
      | Self::TickDriver(err) => write!(f, "tick driver provisioning failed: {err}"),
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
