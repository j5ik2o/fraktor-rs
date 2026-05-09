//! Error type for ask operations initiated from an actor context.

use core::fmt::{self, Formatter, Result as FmtResult};

use fraktor_actor_core_rs::actor::error::{PipeSpawnError, SendError};

/// Describes failures that can occur when invoking ask from a typed actor context.
#[derive(Debug)]
pub enum AskOnContextError {
  /// The request message could not be delivered to the target.
  SendFailed(SendError),
  /// The pipe task could not be spawned to deliver the result.
  PipeSpawnFailed(PipeSpawnError),
}

impl fmt::Display for AskOnContextError {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    match self {
      | AskOnContextError::SendFailed(error) => write!(f, "ask send failed: {error:?}"),
      | AskOnContextError::PipeSpawnFailed(error) => write!(f, "ask pipe spawn failed: {error}"),
    }
  }
}

impl From<SendError> for AskOnContextError {
  fn from(error: SendError) -> Self {
    AskOnContextError::SendFailed(error)
  }
}

impl From<PipeSpawnError> for AskOnContextError {
  fn from(error: PipeSpawnError) -> Self {
    AskOnContextError::PipeSpawnFailed(error)
  }
}
